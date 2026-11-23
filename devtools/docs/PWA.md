# Mod UI as an Android app (PWA) and moving the cards

## Rearranging cards
Every card has a grip (three dots) at the right of its header. Press and drag the grip up or down; the card slides into place and the order
is remembered in that browser (localStorage `modui.order`). Normal scrolling and taps are untouched, because only the grip starts a drag.
With a keyboard, focus the grip and press Up or Down. "Reset layout" (top right) puts the cards back in the original order. Collapsed or
open state is separate and unchanged. Cards that appear later (new extras) are placed next to the card that precedes them on the page.

## Installing on an Android phone
Chrome only offers "Install app" on a secure page, and `http://192.168.x.x` is not one. Simplest route (no certificate, nothing to run):

1. Start the panel with `--lan` as usual; note the address it prints, e.g. `http://192.168.1.20:8765/?key=...`. Open that on the phone once so
   the key is stored.
2. On the phone, in Chrome, open `chrome://flags/#unsafely-treat-insecure-origin-as-secure`.
3. Set it to Enabled and type the panel's origin without the key or path: `http://192.168.1.20:8765` (use your PC's address; more than one
   origin can be listed with commas). Tap "Relaunch".
4. Open the panel address again. Chrome menu (three dots) > "Install app" (or "Add to Home screen" > "Install"). Mod UI appears on the home
   screen with its own icon and opens full screen.
5. If the PC's address changes (new DHCP lease), repeat step 3 with the new origin, or give the PC a fixed address in the router.
   Installed apps remember the key from step 1 (it lives in the page's own storage); if you restart the panel with a new key, open the
   `?key=...` address once in the app's browser tab, or start the panel with a fixed `--key`.

The flag is a Chrome developer setting: it only affects that one origin. The page works without installing, as before.

### Optional: https
`start.py ... --lan --https` serves https with a self-signed certificate it makes once in `SESSION\notes` (needs openssl, which Git for
Windows ships; or pass `--cert FILE --cert-key FILE`). Chrome on Android shows a certificate warning for it and will not install a page
with a warning, so this only works if the certificate is installed on the phone (Settings > Security > Encryption and credentials >
Install a certificate > CA certificate, then pick `panel-cert.pem`). The flag route above is simpler; https is there for a phone you
cannot put the flag on. Tested on the PC only (the server answers over https); not tried on a phone.

## What is cached
`sw.js` caches only the static shell (`/`, `/layout.js`, the manifest and the two icons), network first, so an update always wins and the
app opens offline to a page that says the panel server is not answering. `/state`, `/flags`, every POST and the extras are never
cached or touched. The shell files are served without the access key (they hold nothing private); every data request still needs it.

## Files
Panel: `devtools/panel/static/` (`layout.js`, `sw.js`, `manifest.webmanifest`, `icon-192.png`, `icon-512.png`, `make_icons.py`
draws the icon: no game art), `index.html`, `server.py` (`--https`, `--cert`, `--cert-key`). Tests: `python -I tests/test_pwa.py` (needs node).
