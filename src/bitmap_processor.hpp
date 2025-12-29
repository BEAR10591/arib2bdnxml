#ifndef BITMAP_PROCESSOR_HPP
#define BITMAP_PROCESSOR_HPP

#include <string>
#include <vector>
#include <cstdint>

extern "C" {
#include <libavutil/frame.h>
#include <png.h>
}

struct BitmapData {
    std::vector<uint8_t> data;  // RGBA データ
    int width = 0;
    int height = 0;
    int stride = 0;  // 1行のバイト数
};

class BitmapProcessor {
public:
    // AVFrame からビットマップデータを抽出
    static bool extract_bitmap_from_frame(AVFrame* frame, BitmapData& bitmap);
    
    // ビットマップを PNG ファイルに保存
    static bool save_bitmap_as_png(const BitmapData& bitmap, const std::string& filename);
    
    // PNG ファイル名を生成
    static std::string generate_png_filename(int index, const std::string& base_name = "subtitle");
    
private:
    // PNG 書き込み用のコールバック
    static void png_write_data(png_structp png_ptr, png_bytep data, png_size_t length);
    static void png_flush_data(png_structp png_ptr);
};

#endif // BITMAP_PROCESSOR_HPP

