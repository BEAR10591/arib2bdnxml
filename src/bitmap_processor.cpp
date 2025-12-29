#include "bitmap_processor.hpp"
#include <iostream>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <cstring>

extern "C" {
#include <png.h>
}

// PNG 書き込み用のストリーム構造
struct PngWriteStream {
    std::ofstream* file;
};

void BitmapProcessor::png_write_data(png_structp png_ptr, png_bytep data, png_size_t length) {
    PngWriteStream* stream = static_cast<PngWriteStream*>(png_get_io_ptr(png_ptr));
    stream->file->write(reinterpret_cast<const char*>(data), length);
}

void BitmapProcessor::png_flush_data(png_structp png_ptr) {
    PngWriteStream* stream = static_cast<PngWriteStream*>(png_get_io_ptr(png_ptr));
    stream->file->flush();
}

bool BitmapProcessor::extract_bitmap_from_frame(AVFrame* frame, BitmapData& bitmap) {
    if (!frame) {
        std::cerr << "警告: フレームが NULL です。" << std::endl;
        return false;
    }
    
    // libaribcaption デコーダーはビットマップを AVFrame の data[0] に出力
    // ただし、フォーマットによっては data[0] が NULL の可能性がある
    if (!frame->data[0]) {
        std::cerr << "警告: フレームデータが NULL です。幅: " << frame->width << ", 高さ: " << frame->height << std::endl;
        return false;
    }
    
    if (frame->width <= 0 || frame->height <= 0) {
        std::cerr << "警告: 無効なフレームサイズ: " << frame->width << "x" << frame->height << std::endl;
        return false;
    }
    
    bitmap.width = frame->width;
    bitmap.height = frame->height;
    bitmap.stride = frame->linesize[0];
    
    if (bitmap.stride <= 0) {
        std::cerr << "警告: 無効なストライド: " << bitmap.stride << std::endl;
        return false;
    }
    
    // データをコピー
    size_t data_size = bitmap.stride * bitmap.height;
    bitmap.data.resize(data_size);
    std::memcpy(bitmap.data.data(), frame->data[0], data_size);
    
    return true;
}

bool BitmapProcessor::save_bitmap_as_png(const BitmapData& bitmap, const std::string& filename) {
    if (bitmap.data.empty() || bitmap.width <= 0 || bitmap.height <= 0) {
        std::cerr << "エラー: 無効なビットマップデータです。" << std::endl;
        return false;
    }
    
    std::ofstream file(filename, std::ios::binary);
    if (!file.is_open()) {
        std::cerr << "エラー: ファイルを開けませんでした: " << filename << std::endl;
        return false;
    }
    
    // PNG 構造体を作成
    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    if (!png_ptr) {
        std::cerr << "エラー: PNG 構造体を作成できませんでした。" << std::endl;
        return false;
    }
    
    png_infop info_ptr = png_create_info_struct(png_ptr);
    if (!info_ptr) {
        png_destroy_write_struct(&png_ptr, nullptr);
        std::cerr << "エラー: PNG 情報構造体を作成できませんでした。" << std::endl;
        return false;
    }
    
    // エラーハンドリング
    if (setjmp(png_jmpbuf(png_ptr))) {
        png_destroy_write_struct(&png_ptr, &info_ptr);
        std::cerr << "エラー: PNG 書き込み中にエラーが発生しました。" << std::endl;
        return false;
    }
    
    // 書き込みストリームを設定
    PngWriteStream stream;
    stream.file = &file;
    png_set_write_fn(png_ptr, &stream, png_write_data, png_flush_data);
    
    // PNG 情報を設定
    png_set_IHDR(png_ptr, info_ptr, bitmap.width, bitmap.height, 8,
                 PNG_COLOR_TYPE_RGBA, PNG_INTERLACE_NONE,
                 PNG_COMPRESSION_TYPE_DEFAULT, PNG_FILTER_TYPE_DEFAULT);
    
    // 画像データを設定
    std::vector<png_bytep> row_pointers(bitmap.height);
    for (int y = 0; y < bitmap.height; y++) {
        row_pointers[y] = const_cast<png_bytep>(bitmap.data.data() + y * bitmap.stride);
    }
    
    png_set_rows(png_ptr, info_ptr, row_pointers.data());
    png_write_png(png_ptr, info_ptr, PNG_TRANSFORM_IDENTITY, nullptr);
    
    // リソースを解放
    png_destroy_write_struct(&png_ptr, &info_ptr);
    
    return true;
}

std::string BitmapProcessor::generate_png_filename(int index, const std::string& base_name) {
    std::ostringstream oss;
    oss << base_name << std::setfill('0') << std::setw(5) << index << ".png";
    return oss.str();
}

