project('arib2bdnxml', 'cpp',
  version : '0.1.0',
  default_options : [
    'cpp_std=c++17',
    'warning_level=2',
    'werror=false',
  ])

# 依存関係
ffmpeg_dep = dependency('libavcodec', version: '>=58.0.0', required: true)
ffmpeg_format_dep = dependency('libavformat', version: '>=58.0.0', required: true)
ffmpeg_util_dep = dependency('libavutil', version: '>=56.0.0', required: true)
ffmpeg_swscale_dep = dependency('libswscale', version: '>=5.0.0', required: true)
png_dep = dependency('libpng', required: true)

# ソースファイル
src_files = files(
  'src/main.cpp',
  'src/options.cpp',
  'src/ffmpeg_wrapper.cpp',
  'src/bitmap_processor.cpp',
  'src/bdn_xml_generator.cpp',
)

# 実行ファイル
executable('arib2bdnxml',
  src_files,
  dependencies: [
    ffmpeg_dep,
    ffmpeg_format_dep,
    ffmpeg_util_dep,
    ffmpeg_swscale_dep,
    png_dep,
  ],
  install: true,
)

