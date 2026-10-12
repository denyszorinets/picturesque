libvips 8.18.3 · LibRaw 0.22.2-Release

## RAW: 512 px thumbnail (median of 3 warm runs)

| File | A: vips dcrawload | B: LibRaw preview + vips | C: rawler preview + image | LibRaw unpack | rawler decode |
|---|---|---|---|---|---|
| Apple_iPhone_12_Pro_IMG_1361.DNG | 651 ms (384x512) | 73 ms (4032x3024 jpeg → 384x512) | 393 ms (4032x3024 → 512x384) | ✅ | ✅ |
| Canon_Canon_EOS_R6_Mark_III_IMG_4241.CR3 | 675 ms (512x341) | 61 ms (6960x4640 jpeg → 512x341) | 144 ms (6960x4640 → 512x341) | ✅ | ✅ |
| Canon_EOS_5D_Mark_IV_B13A0729.CR2 | 975 ms (512x342) | 49 ms (6720x4480 jpeg → 512x341) | 126 ms (6720x4480 → 512x341) | ✅ | ✅ |
| Canon_EOS_R5_Canon_EOS_R5_CRAW_ISO_100_crop_dual | 385 ms (512x341) | 32 ms (5088x3392 jpeg → 512x341) | 75 ms (5088x3392 → 512x341) | ✅ | ✅ |
| Fujifilm_X-T3_AFXT2720.RAF | 1998 ms (512x342) | 44 ms (4416x2944 jpeg → 512x341) | 77 ms (4416x2944 → 512x341) | ✅ | ✅ |
| Fujifilm_X-T5_DSCF0021.RAF | 3205 ms (512x342) | 52 ms (4416x2944 jpeg → 512x341) | 88 ms (4416x2944 → 512x341) | ✅ | ✅ |
| Google_Pixel_2_XL_IMG_20180109_080629.dng | 308 ms (512x384) | ❌ unpack_thumb: No thumbnail in file (-5) | 349 ms (4016x3008 → 512x383) | ✅ | ✅ |
| Leica_Q2_L1000750.DNG | 1295 ms (512x344) | 51 ms (8368x5584 jpeg → 512x342) | 15 ms (1620x1080 → 512x341) | ✅ | ✅ |
| Nikon_D850_Nikon-D850-12bit-compressed.NEF | 1432 ms (512x341) | 67 ms (8256x5504 jpeg → 512x341) | 207 ms (8256x5504 → 512x341) | ✅ | ✅ |
| Nikon_Z_8_Nikon_Z8_high_efficiency_low.NEF | ❌ vips decode: dcrawload: unable to unpack: Unsupported file f | 66 ms (8256x5504 jpeg → 512x341) | 207 ms (8256x5504 → 512x341) | ❌ unpack: Unsupported file format or not RAW file (-2) | ❌ Failed to decode image, possibly corrupt image: NE |
| OM_Digital_Solutions_OM-3_P5048182.ORF | 791 ms (512x384) | 26 ms (3200x2400 jpeg → 512x384) | 817 ms (5184x3888 → 512x384) | ✅ | ✅ |
| Olympus_E-M1MarkII_Olympus_EM1mk2_Standard_20MP. | 706 ms (512x382) | 28 ms (3200x2400 jpeg → 512x384) | 758 ms (5184x3888 → 512x384) | ✅ | ✅ |
| Panasonic_DC-GH5__T012010.RW2 | 579 ms (512x384) | 20 ms (1920x1440 jpeg → 512x384) | 20 ms (1920x1440 → 512x384) | ✅ | ✅ |
| Pentax_K-1_IMGP8550.PEF | 1095 ms (512x342) | 67 ms (7360x4912 jpeg → 512x342) | 7 ms (720x480 → 512x341) | ✅ | ✅ |
| Sony_ILCE-7M3_UNCOMPRESSED_14bit.ARW | 485 ms (512x342) | 21 ms (1616x1080 jpeg → 512x342) | 20 ms (1616x1080 → 512x342) | ✅ | ✅ |
| Sony_ILCE-7RM5_7RM5-LosslessCompressedLarge.ARW | 1816 ms (512x341) | 106 ms (9504x6336 jpeg → 512x341) | 16 ms (1616x1080 → 512x342) | ✅ | ✅ |

## JPEG / PNG / TIFF / WebP: 512 px thumbnail (median of 3 warm runs)

| File | Size | A/B: vips thumbnail | C: image decode + thumbnail |
|---|---|---|---|
| photo.jpg | 9504x6336, 11.5 MB | 99 ms (512x341) | 293 ms (512x341) |
| photo.png | 9504x6336, 146.6 MB | 479 ms (512x341) | 534 ms (512x341) |
| photo.tif | 9504x6336, 180.7 MB | 55 ms (512x341) | 248 ms (512x341) |
| photo.webp | 9504x6336, 6.3 MB | 236 ms (512x341) | 1146 ms (512x341) |

## EXIF orientation (difference from upright reference; < 10 is correct)

| File | vips thumbnail | image + apply_orientation |
|---|---|---|
| Landscape_1.jpg | ✅ 0.2 | ✅ 0.0 |
| Landscape_2.jpg | ✅ 0.4 | ✅ 0.2 |
| Landscape_3.jpg | ✅ 0.4 | ✅ 0.2 |
| Landscape_4.jpg | ✅ 0.3 | ✅ 0.1 |
| Landscape_5.jpg | ✅ 0.4 | ✅ 0.1 |
| Landscape_6.jpg | ✅ 0.4 | ✅ 0.2 |
| Landscape_7.jpg | ✅ 0.3 | ✅ 0.2 |
| Landscape_8.jpg | ✅ 0.4 | ✅ 0.2 |

## HEIC

| File | vips thumbnail | libheif-rs decode |
|---|---|---|
| example.heic | ❌ vips decode: corpus/heic/example.heic: bad seek to 718130 | ❌ PluginLoadingError(NoMatchingDecoderInstalled) Error while l |

