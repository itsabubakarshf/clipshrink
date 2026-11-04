"""Regenerates assets/icon.png and assets/icon.ico (run: python3 tools/make_icon.py). Needs Pillow."""
from PIL import Image, ImageDraw, ImageFilter

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

# gradient background (blue -> indigo), rounded square
grad = Image.new("RGBA", (S, S))
px = grad.load()
c1, c2 = (14, 165, 233), (79, 70, 229)
for y in range(S):
    for x in range(S):
        t = (x + y) / (2 * S - 2)
        px[x, y] = tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3)) + (255,)
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle((32, 32, S - 32, S - 32), radius=230, fill=255)
img.paste(grad, (0, 0), mask)

d = ImageDraw.Draw(img)
# film-strip perforations on both sides
for i in range(5):
    y = 150 + i * 165
    for x in (96, S - 96 - 70):
        d.rounded_rectangle((x, y, x + 70, y + 90), radius=18, fill=(255, 255, 255, 70))

# soft shadow + white rounded play triangle
tri = [(380, 290), (380, 734), (760, 512)]
shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(shadow).polygon([(x + 0, y + 22) for x, y in tri], fill=(10, 20, 80, 120))
img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(22)))
layer = Image.new("L", (S, S), 0)
ImageDraw.Draw(layer).polygon(tri, fill=255)
# round the corners: blur then threshold
layer = layer.filter(ImageFilter.GaussianBlur(26)).point(lambda v: 255 if v > 150 else 0)
layer = layer.filter(ImageFilter.GaussianBlur(1.5))
white = Image.new("RGBA", (S, S), (255, 255, 255, 255))
img.paste(white, (0, 0), layer)

# "squeeze" cue: two small arrows pointing inward under the triangle
d = ImageDraw.Draw(img)
for cx, direction in ((300, 1), (724, -1)):
    y = 850
    d.line((cx - 60 * direction, y, cx + 20 * direction, y), fill=(255, 255, 255, 230), width=34)
    d.polygon([(cx + 20 * direction, y - 46), (cx + 80 * direction, y), (cx + 20 * direction, y + 46)], fill=(255, 255, 255, 230))

img.resize((256, 256), Image.LANCZOS).save("assets/icon.png")
sizes = [16, 24, 32, 48, 64, 128, 256]
img.resize((256, 256), Image.LANCZOS).save("assets/icon.ico", sizes=[(s, s) for s in sizes])
img.resize((512, 512), Image.LANCZOS).save("assets/icon-preview.png")
