# Convert bmp

The spritesheet bpms are created in aseprite as 24bit.
Convert them to 1 bit monochrome to save space.

`ffmpeg -i source.bmp -pix_fmt monob target_1bit.bmp -y`