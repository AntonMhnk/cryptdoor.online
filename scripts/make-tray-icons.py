#!/usr/bin/env python3
"""Generate macOS menu-bar tray icons for CryptDoor.

Pure white silhouette of our circle-lock with a keyhole, at 44px (@2x).
Two states:
  tray-on.png  — full white, solid (active VPN)
  tray-off.png — full white, but only outline (idle VPN)
"""

from pathlib import Path
from PIL import Image, ImageDraw


SIZE = 44  # @2x; macOS will render at 22pt


def make_icon(connected: bool, out: Path) -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    cx = SIZE / 2
    cy = SIZE / 2
    pad = 4
    r_outer = (SIZE - pad * 2) / 2     # 18
    r_inner = r_outer - 4               # 14

    white = (255, 255, 255, 255)

    if connected:
        # Solid filled disc, then cut out a thin ring + keyhole
        draw.ellipse(
            (cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer),
            fill=white,
        )
        # Inner hollow ring (visual detail like our app icon)
        draw.ellipse(
            (cx - r_inner, cy - r_inner, cx + r_inner, cy + r_inner),
            fill=(0, 0, 0, 0),
        )
        # Re-fill inner core so keyhole sits on solid white
        r_core = r_inner - 2
        draw.ellipse(
            (cx - r_core, cy - r_core, cx + r_core, cy + r_core),
            fill=white,
        )
        # Keyhole cutout (transparent)
        kh_r = 3
        draw.ellipse((cx - kh_r, cy - kh_r - 2, cx + kh_r, cy + kh_r - 2), fill=(0, 0, 0, 0))
        draw.polygon(
            [
                (cx - 2, cy - 1),
                (cx + 2, cy - 1),
                (cx + 1.5, cy + 6),
                (cx - 1.5, cy + 6),
            ],
            fill=(0, 0, 0, 0),
        )
    else:
        # Outline only: ring + keyhole drawn in white
        draw.ellipse(
            (cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer),
            outline=white,
            width=2,
        )
        # Keyhole (still white, outlined)
        kh_r = 3
        draw.ellipse(
            (cx - kh_r, cy - kh_r - 2, cx + kh_r, cy + kh_r - 2),
            outline=white,
            width=2,
        )
        draw.line([(cx, cy + 1), (cx, cy + 7)], fill=white, width=2)

    img.save(out, format="PNG")
    print(f"wrote {out}")


def main() -> None:
    out_dir = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"
    out_dir.mkdir(parents=True, exist_ok=True)
    make_icon(connected=True, out=out_dir / "tray-on.png")
    make_icon(connected=False, out=out_dir / "tray-off.png")


if __name__ == "__main__":
    main()
