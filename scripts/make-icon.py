#!/usr/bin/env python3
"""Crop AI-generated source to 1024x1024 and apply macOS-style squircle mask
with proper safe-area padding. Outputs ./icon-1024.png next to source."""

import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter


def squircle_mask(size: int, radius_factor: float = 0.225) -> Image.Image:
    """macOS Big Sur+ uses ~22.37% of side as corner radius (a true squircle is
    smoother but this is the canonical approximation Apple uses for app icons)."""
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    r = int(size * radius_factor)
    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=r, fill=255)
    return mask


def main(src_path: str, out_path: str, final_size: int = 1024) -> None:
    src = Image.open(src_path).convert("RGBA")
    w, h = src.size

    # 1. Center-crop to square
    side = min(w, h)
    left = (w - side) // 2
    top = (h - side) // 2
    cropped = src.crop((left, top, left + side, top + side))

    # 2. Resize to slightly larger than final to allow inset padding (safe area)
    safe = int(final_size * 0.86)  # icon body fills 86% of canvas
    body = cropped.resize((safe, safe), Image.LANCZOS)

    # 3. Apply squircle mask to body
    mask = squircle_mask(safe)
    masked_body = Image.new("RGBA", (safe, safe), (0, 0, 0, 0))
    masked_body.paste(body, (0, 0), mask)

    # 4. Compose onto transparent canvas, centered (this leaves macOS safe area)
    canvas = Image.new("RGBA", (final_size, final_size), (0, 0, 0, 0))
    inset = (final_size - safe) // 2
    canvas.paste(masked_body, (inset, inset), masked_body)

    # 5. Add subtle drop shadow under icon for depth (macOS-style)
    shadow_layer = Image.new("RGBA", (final_size, final_size), (0, 0, 0, 0))
    shadow_mask = Image.new("L", (final_size, final_size), 0)
    sd_draw = ImageDraw.Draw(shadow_mask)
    sd_r = int(safe * 0.225)
    sd_draw.rounded_rectangle(
        (inset, inset + int(safe * 0.02), inset + safe - 1, inset + safe - 1 + int(safe * 0.02)),
        radius=sd_r,
        fill=110,
    )
    shadow_mask = shadow_mask.filter(ImageFilter.GaussianBlur(radius=int(final_size * 0.025)))
    shadow_layer.putalpha(shadow_mask)

    out = Image.alpha_composite(shadow_layer, canvas)
    out.save(out_path, format="PNG")
    print(f"wrote {out_path} ({final_size}x{final_size})")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("usage: make-icon.py <source.png> <out.png>", file=sys.stderr)
        sys.exit(1)
    main(sys.argv[1], sys.argv[2])
