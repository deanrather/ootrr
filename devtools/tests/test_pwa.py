"""Mod UI: the install-as-an-app files and the card drag-order logic. python -I -m unittest test_pwa  (needs node for the JS parts)"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402

PANEL = _paths.PANEL
STATIC = os.path.join(PANEL, "static")
NODE = shutil.which("node")
PORT = 8791


def node(code):
    r = subprocess.run([NODE, "-e", code], capture_output=True, text=True, timeout=30)
    if r.returncode:
        raise AssertionError(r.stderr)
    return json.loads(r.stdout)


def get(path, port=PORT, headers=None):
    try:
        r = urllib.request.urlopen(urllib.request.Request("http://127.0.0.1:%d%s" % (port, path), headers=headers or {}), timeout=10)
        return r.status, r.headers, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.headers, e.read()


@unittest.skipUnless(NODE, "node not installed")
class Order(unittest.TestCase):
    def js(self, expr):
        return node("global.window=undefined;const L=require(%s);console.log(JSON.stringify(%s))" % (json.dumps(os.path.join(STATIC, "layout.js")), expr))

    def test_normalize_drops_unknown_and_appends_new(self):
        self.assertEqual(self.js("L.normalize(['c','zz','a','a'],['a','b','c','d'])"), ["c", "a", "b", "d"])

    def test_normalize_survives_garbage(self):
        self.assertEqual(self.js("L.normalize('x',['a','b'])"), ["a", "b"])

    def test_place_puts_new_cards_by_their_page_neighbour(self):
        self.assertEqual(self.js("L.place(['b','a'],['a','x','b'])"), ["b", "a", "x"])
        self.assertEqual(self.js("L.place([],['a','b','c'])"), ["a", "b", "c"])
        self.assertEqual(self.js("L.place(['c','zz'],['a','c'])"), ["a", "c"])

    def test_move(self):
        self.assertEqual(self.js("L.moveTo(['a','b','c'],'a',2)"), ["b", "c", "a"])
        self.assertEqual(self.js("L.moveTo(['a','b','c'],'c',0)"), ["c", "a", "b"])
        self.assertEqual(self.js("L.moveTo(['a','b','c'],'b',99)"), ["a", "c", "b"])

    def test_target_index(self):
        self.assertEqual(self.js("L.targetIndex([10,50,90],60)"), 2)
        self.assertEqual(self.js("L.targetIndex([10,50,90],5)"), 0)

    def test_storage_failure_is_harmless(self):
        out = node("global.window=undefined;global.localStorage={getItem(){throw 1},setItem(){throw 1},removeItem(){throw 1}};"
                   "const L=require(%s);L.save(['a']);L.forget();console.log(JSON.stringify(L.load()))" % json.dumps(os.path.join(STATIC, "layout.js")))
        self.assertEqual(out, [])

    def test_storage_round_trip(self):
        out = node("global.window=undefined;const m={};global.localStorage={getItem:k=>m[k]||null,setItem:(k,v)=>{m[k]=v},removeItem:k=>{delete m[k]}};"
                   "const L=require(%s);L.save(['b','a']);console.log(JSON.stringify(L.load()))" % json.dumps(os.path.join(STATIC, "layout.js")))
        self.assertEqual(out, ["b", "a"])


@unittest.skipUnless(NODE, "node not installed")
class ServiceWorker(unittest.TestCase):
    def run_fetch(self, url, method="GET"):
        code = r"""
const listeners={};
global.self={location:new URL('http://panel.test/'),addEventListener:(t,f)=>{listeners[t]=f},skipWaiting(){},clients:{claim(){}}};
global.caches={open:async()=>({put(){},add(){}}),match:async()=>null,keys:async()=>[],delete:async()=>1};
global.fetch=async()=>({ok:true,clone(){return this}});
require(%s);
let answered=false;
listeners.fetch({request:{url:%s,method:%s},respondWith:()=>{answered=true}});
console.log(JSON.stringify(answered));
""" % (json.dumps(os.path.join(STATIC, "sw.js")), json.dumps(url), json.dumps(method))
        return node(code)

    def test_shell_is_handled(self):
        for p in ("/", "/layout.js", "/manifest.webmanifest", "/icon-192.png", "/icon-512.png"):
            self.assertTrue(self.run_fetch("http://panel.test" + p), p)

    def test_api_and_state_are_never_touched(self):
        for p in ("/state", "/flags?since=3", "/destinations", "/extras/actors.html", "/button", "/shots/a.png"):
            self.assertFalse(self.run_fetch("http://panel.test" + p), p)

    def test_post_and_other_origin_are_never_touched(self):
        self.assertFalse(self.run_fetch("http://panel.test/", "POST"))
        self.assertFalse(self.run_fetch("http://other.test/layout.js"))


class Served(unittest.TestCase):
    proc = None

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp()
        cls.proc = subprocess.Popen([sys.executable, "-I", os.path.join(PANEL, "server.py"), "--sock", "none", "--session", cls.tmp,
                                     "--port", str(PORT), "--steps", os.path.join(cls.tmp, "s.json"), "--title", "Mod UI"],
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                get("/manifest.webmanifest")
                break
            except OSError:
                time.sleep(0.2)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(10)
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def test_manifest(self):
        code, h, body = get("/manifest.webmanifest")
        self.assertEqual(code, 200)
        self.assertIn("application/manifest+json", h["Content-Type"])
        m = json.loads(body)
        self.assertEqual((m["name"], m["display"], m["start_url"]), ("Mod UI", "standalone", "/"))
        sizes = {i["sizes"] for i in m["icons"]}
        self.assertTrue({"192x192", "512x512"} <= sizes)
        for i in m["icons"]:
            self.assertEqual(get(i["src"])[0], 200)

    def test_icons_are_pngs_of_the_stated_size(self):
        import struct
        for n in (192, 512):
            code, h, body = get("/icon-%d.png" % n)
            self.assertEqual((code, h["Content-Type"]), (200, "image/png"))
            self.assertEqual(body[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(struct.unpack(">II", body[16:24]), (n, n))

    def test_service_worker_and_script(self):
        code, h, body = get("/sw.js")
        self.assertEqual(code, 200)
        self.assertIn("javascript", h["Content-Type"])
        self.assertIn(b"fetch", body)
        self.assertEqual(get("/layout.js")[0], 200)

    def test_page_links_the_manifest_and_registers(self):
        code, h, body = get("/")
        self.assertEqual(code, 200)
        for needle in (b'rel="manifest"', b"serviceWorker", b"/layout.js", b"resetlayout"):
            self.assertIn(needle, body)

    def test_state_still_refuses_nothing_static_leaks(self):
        self.assertEqual(get("/static/sw.js")[0], 404)
        self.assertEqual(get("/make_icons.py")[0], 404)
        self.assertEqual(get("/../server.py")[0], 404)


if __name__ == "__main__":
    unittest.main()
