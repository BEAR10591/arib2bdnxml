#include "options.hpp"
#include "ffmpeg_wrapper.hpp"
#include "bitmap_processor.hpp"
#include "bdn_xml_generator.hpp"
#include <iostream>
#include <filesystem>
#include <vector>
#include <memory>

namespace fs = std::filesystem;

// デバッグログ用マクロ（main関数内でopts.debugを使用）
#define DEBUG_LOG(x) do { if (opts.debug) std::cout << x << std::endl; } while(0)

int main(int argc, char* argv[]) {
    // オプションを解析
    Options opts = parse_options(argc, argv);
    
    // 入力ファイルの存在確認
    if (!fs::exists(opts.input_file)) {
        std::cerr << "エラー: 入力ファイルが存在しません: " << opts.input_file << std::endl;
        return 1;
    }
    
    // ベースファイル名を取得
    std::string base_name = fs::path(opts.input_file).stem().string();
    
    // 出力ディレクトリを決定
    std::string output_dir;
    if (opts.output_dir.has_value()) {
        output_dir = *opts.output_dir;
    } else {
        // デフォルト: 入力ファイルと同じディレクトリに<動画ファイル名>_bdnxmlを作成
        fs::path input_path = fs::path(opts.input_file);
        output_dir = (input_path.parent_path() / (base_name + "_bdnxml")).string();
    }
    
    // 出力ディレクトリを作成
    if (!fs::exists(output_dir)) {
        fs::create_directories(output_dir);
    }
    
    // FFmpeg ラッパーを作成
    FFmpegWrapper ffmpeg;
    ffmpeg.set_debug(opts.debug);
    
    if (opts.debug) {
        DEBUG_LOG("FFmpeg ラッパーを作成中...");
    }
    
    // ファイルを開く
    if (opts.debug) {
        DEBUG_LOG("ファイルを開いています: " << opts.input_file);
    }
    if (!ffmpeg.open_file(opts.input_file)) {
        std::cerr << "エラー: ファイルを開けませんでした。" << std::endl;
        return 1;
    }
    if (opts.debug) {
        DEBUG_LOG("ファイルを開きました。");
    }
    
    // 動画情報を取得
    if (opts.debug) {
        DEBUG_LOG("動画情報を取得中...");
    }
    VideoInfo video_info = ffmpeg.get_video_info();
    if (opts.debug) {
        DEBUG_LOG("動画情報: " << video_info.width << "x" << video_info.height 
                  << ", FPS: " << video_info.fps << ", start_time: " << video_info.start_time);
    }
    
    // canvas_size の決定
    std::map<std::string, std::string> libaribcaption_opts = opts.libaribcaption_opts;
    std::string canvas_size;
    
    // --resolution オプションが指定されている場合
    if (opts.resolution.has_value()) {
        std::string resolution = *opts.resolution;
        // 選択肢を検証
        if (resolution == "1920x1080" || resolution == "1440x1080" || 
            resolution == "1280x720" || resolution == "720x480") {
            canvas_size = resolution;
            DEBUG_LOG("canvas_size を --resolution オプションから取得: " << canvas_size);
        } else {
            std::cerr << "エラー: 無効な解像度: " << resolution << std::endl;
            std::cerr << "有効な解像度: 1920x1080, 1440x1080, 1280x720, 720x480" << std::endl;
            return 1;
        }
    } else {
        // --resolution が指定されていない場合、動画解像度に基づいて自動決定
        int video_width = video_info.width;
        int video_height = video_info.height;
        
        if (video_width == 1920 && video_height == 1080) {
            canvas_size = "1920x1080";
        } else if (video_width == 1440 && video_height == 1080) {
            canvas_size = "1920x1080";
        } else if (video_width == 1280 && video_height == 720) {
            canvas_size = "1280x720";
        } else if (video_width == 720 && video_height == 480) {
            canvas_size = "720x480";
        } else {
            std::cerr << "エラー: サポートされていない動画解像度: " << video_width << "x" << video_height << std::endl;
            std::cerr << "サポートされている解像度: 1920x1080, 1440x1080, 1280x720, 720x480" << std::endl;
            std::cerr << "--resolution オプションで解像度を指定してください。" << std::endl;
            return 1;
        }
        DEBUG_LOG("canvas_size を動画解像度から自動決定: " << canvas_size);
    }
    
    // canvas_size を libaribcaption_opts に設定
    libaribcaption_opts["canvas_size"] = canvas_size;
    
    // libaribcaption のデフォルト値を設定（ユーザーが明示的に指定していない場合のみ）
    if (libaribcaption_opts.find("outline_width") == libaribcaption_opts.end()) {
        libaribcaption_opts["outline_width"] = "0.0";
    }
    if (libaribcaption_opts.find("replace_msz_ascii") == libaribcaption_opts.end()) {
        libaribcaption_opts["replace_msz_ascii"] = "0";
    }
    if (libaribcaption_opts.find("replace_msz_japanese") == libaribcaption_opts.end()) {
        libaribcaption_opts["replace_msz_japanese"] = "0";
    }
    if (libaribcaption_opts.find("replace_drcs") == libaribcaption_opts.end()) {
        libaribcaption_opts["replace_drcs"] = "0";
    }
    
    // canvas_size を解析して BDN 情報を設定
    size_t x_pos = canvas_size.find('x');
    if (x_pos == std::string::npos) {
        std::cerr << "エラー: 無効な canvas_size 形式: " << canvas_size << std::endl;
        return 1;
    }
    int canvas_width = std::stoi(canvas_size.substr(0, x_pos));
    int canvas_height = std::stoi(canvas_size.substr(x_pos + 1));
    
    // BDN 情報を設定
    BDNInfo bdn_info;
    bdn_info.video_width = canvas_width;
    bdn_info.video_height = canvas_height;
    bdn_info.fps = video_info.fps > 0 ? video_info.fps : 29.97;
    
    // libaribcaption デコーダーを初期化（解像度決定後）
    DEBUG_LOG("デコーダーを初期化中...");
    if (!ffmpeg.init_decoder(libaribcaption_opts)) {
        std::cerr << "エラー: デコーダーを初期化できませんでした。" << std::endl;
        ffmpeg.close();
        return 1;
    }
    DEBUG_LOG("デコーダーを初期化しました。");
    
    // BDN XML ジェネレーターを作成
    BDNXmlGenerator generator(bdn_info);
    
    // 字幕フレームを処理
    DEBUG_LOG("字幕フレームの処理を開始します...");
    int frame_index = 0;
    std::vector<SubtitleEvent> events;
    
    SubtitleFrame subtitle_frame;
    SubtitleFrame next_subtitle_frame;
    bool has_next_frame = false;
    
    DEBUG_LOG("最初の字幕フレームを取得中...");
    
    // 最初のフレームを取得
    if (!ffmpeg.get_next_subtitle_frame(subtitle_frame)) {
        DEBUG_LOG("字幕フレームが見つかりませんでした。");
    } else {
        // 次のフレームを先読み
        has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
        
        do {
            DEBUG_LOG("字幕フレームを取得しました: インデックス " << frame_index);
            
            // 消去コマンド（bitmap == nullptr かつ timestamp が設定されている）の場合
            if (!subtitle_frame.bitmap && subtitle_frame.timestamp > 0) {
                // 直前の字幕イベントの終了時間を更新
                if (!events.empty()) {
                    double clear_timestamp = BDNXmlGenerator::adjust_timestamp(
                        subtitle_frame.timestamp, video_info.start_time);
                    events.back().out_tc = BDNXmlGenerator::time_to_tc(clear_timestamp, bdn_info.fps);
                    DEBUG_LOG("消去コマンドを検出: 直前の字幕を終了 - " << events.back().out_tc);
                }
                // 次のフレームに進む
                subtitle_frame = next_subtitle_frame;
                has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
                continue;
            }
            
            if (!subtitle_frame.bitmap) {
                // 次のフレームに進む
                subtitle_frame = next_subtitle_frame;
                has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
                continue;
            }
            
            // ビットマップデータは既に RGBA 形式で取得済み
            BitmapData* bitmap = subtitle_frame.bitmap;
            
            // 空のビットマップはスキップ
            if (bitmap->width == 0 || bitmap->height == 0) {
                delete bitmap;
                // 次のフレームに進む
                subtitle_frame = next_subtitle_frame;
                has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
                continue;
            }
            
            // start_display_time と end_display_time を使用してタイムスタンプを調整（start_time を基準に）
            double start_timestamp;
            double end_timestamp;
            
            // start_display_time と end_display_time が有効な場合は使用
            if (subtitle_frame.start_time > 0 && subtitle_frame.end_time > subtitle_frame.start_time) {
                start_timestamp = BDNXmlGenerator::adjust_timestamp(
                    subtitle_frame.start_time, video_info.start_time);
                end_timestamp = BDNXmlGenerator::adjust_timestamp(
                    subtitle_frame.end_time, video_info.start_time);
                DEBUG_LOG("フレーム " << frame_index << ": start_display_time/end_display_time を使用 - start=" 
                          << start_timestamp << "s, end=" << end_timestamp << "s");
            } else {
                // 無効な場合は、パケットのPTSを使用し、次のフレームの開始時間を終了時間として使用
                start_timestamp = BDNXmlGenerator::adjust_timestamp(
                    subtitle_frame.timestamp, video_info.start_time);
                
                if (has_next_frame && next_subtitle_frame.bitmap) {
                    // 次のフレームのstart_display_time/end_display_timeが有効な場合はそれを使用
                    if (next_subtitle_frame.start_time > 0 && next_subtitle_frame.end_time > next_subtitle_frame.start_time) {
                        double next_start = BDNXmlGenerator::adjust_timestamp(
                            next_subtitle_frame.start_time, video_info.start_time);
                        end_timestamp = next_start;
                    } else {
                        // 次のフレームのtimestampを使用
                        double next_timestamp = BDNXmlGenerator::adjust_timestamp(
                            next_subtitle_frame.timestamp, video_info.start_time);
                        end_timestamp = next_timestamp;
                    }
                } else if (has_next_frame && !next_subtitle_frame.bitmap) {
                    // 次のフレームが消去コマンドの場合、そのtimestampを使用
                    double clear_timestamp = BDNXmlGenerator::adjust_timestamp(
                        next_subtitle_frame.timestamp, video_info.start_time);
                    end_timestamp = clear_timestamp;
                } else {
                    // 次のフレームがない場合、デフォルトで 1 秒の表示時間を設定
                    end_timestamp = start_timestamp + 1.0;
                    DEBUG_LOG("警告: start_display_time/end_display_time が無効で、次のフレームもありません。デフォルトで 1 秒の表示時間を設定: " 
                              << end_timestamp << "s");
                }
                DEBUG_LOG("フレーム " << frame_index << ": timestamp を使用 - start=" 
                          << start_timestamp << "s, end=" << end_timestamp << "s");
            }
            
            // ゼロ期間のグラフィックをスキップ（start_time >= end_time）
            if (start_timestamp >= end_timestamp) {
                DEBUG_LOG("警告: ゼロ期間のグラフィックをスキップします - start=" << start_timestamp 
                          << "s, end=" << end_timestamp << "s, フレーム=" << frame_index);
                delete bitmap;
                // 次のフレームに進む
                subtitle_frame = next_subtitle_frame;
                has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
                continue;
            }
            
            // PNG ファイル名を生成
            std::string png_filename = BitmapProcessor::generate_png_filename(frame_index, base_name);
            std::string png_path = fs::path(output_dir) / png_filename;
            
            // PNG を保存
            if (!BitmapProcessor::save_bitmap_as_png(*bitmap, png_path)) {
                std::cerr << "警告: PNG の保存に失敗しました: " << png_path << std::endl;
                delete bitmap;
                // 次のフレームに進む
                subtitle_frame = next_subtitle_frame;
                has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
                continue;
            }
            
            // 字幕イベントを作成
            SubtitleEvent event;
            event.in_tc = BDNXmlGenerator::time_to_tc(start_timestamp, bdn_info.fps);
            event.out_tc = BDNXmlGenerator::time_to_tc(end_timestamp, bdn_info.fps);
            event.png_file = png_filename;
            event.x = subtitle_frame.x;
            event.y = subtitle_frame.y;
            event.width = bitmap->width;
            event.height = bitmap->height;
            
            events.push_back(event);
            
            frame_index++;
            
            // ビットマップデータを解放
            delete bitmap;
            
            // 次のフレームに進む
            subtitle_frame = next_subtitle_frame;
            has_next_frame = ffmpeg.get_next_subtitle_frame(next_subtitle_frame);
        } while (subtitle_frame.bitmap != nullptr || has_next_frame);
    }
    
    // イベントをジェネレーターに追加
    for (const auto& event : events) {
        generator.add_event(event);
    }
    
    // BDN XML を保存
    std::string xml_path = fs::path(output_dir) / (base_name + ".xml");
    if (!generator.write_to_file(xml_path)) {
        std::cerr << "エラー: BDN XML の保存に失敗しました。" << std::endl;
        ffmpeg.close();
        return 1;
    }
    
    DEBUG_LOG("完了: " << events.size() << " 個の字幕イベントを処理しました。");
    DEBUG_LOG("出力: " << xml_path);
    
    // リソースを解放
    ffmpeg.close();
    
    return 0;
}

