from dataclasses import dataclass

from Options import Choice, FreeText, OptionDict, PerGameCommonOptions

from .tables import presets


def _preset_choice() -> type:
    ps = presets()
    attrs = {"__doc__": "The OOTRR settings preset this world is built from. Each one is a full set of OOTRR settings:\n\n" + "\n".join(f"{p['name']}: {p['tip']}" for p in ps), "display_name": "Preset"}
    for k, p in enumerate(ps):
        attrs[f"option_{p['id']}"] = k
    attrs["default"] = next((k for k, p in enumerate(ps) if p["id"] == "default"), 0)
    return type("Preset", (Choice,), attrs)


Preset = _preset_choice()


class SettingsString(FreeText):
    """An OOTRR settings string, copied from the OOTRR app. When it is set, it replaces the preset.

    Entrance shuffle is not supported yet. Generating a world from a settings string runs the OOTRR app on the computer that generates the room, so that computer needs ootrr_options.ootrr_exe in its host.yaml."""
    display_name = "OOTRR settings string"
    default = ""


class OOTRRPlando(OptionDict):
    """The OOTRR app's Plandomizer: puts one of your own items at one of your own locations, as location: item.

    The OOTRR app writes it into the player file. It works whatever the host's plando setting is."""
    display_name = "OOTRR plandomizer"
    default = {}


@dataclass
class OOTRROptions(PerGameCommonOptions):
    preset: Preset
    settings_string: SettingsString
    ootrr_plando: OOTRRPlando
