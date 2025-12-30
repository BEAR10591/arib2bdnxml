#ifndef OPTIONS_HPP
#define OPTIONS_HPP

#include <string>
#include <vector>
#include <map>
#include <optional>

struct Options {
    // 入力ファイル
    std::string input_file;
    
    // 出力ディレクトリ（省略時は入力ファイルと同じディレクトリに<動画ファイル名>_bdnxmlを作成）
    std::optional<std::string> output_dir;
    
    // 解像度（1920x1080, 1440x1080, 1280x720, 720x480）
    std::optional<std::string> resolution;
    
    // libaribcaption オプション（key=value のペア）
    std::map<std::string, std::string> libaribcaption_opts;
    
    // タイムスタンプ調整用オプション（ffmpeg -ss/-to でカットした動画用）
    std::optional<double> ss;  // 開始時刻（秒単位、ミリ秒まで対応）
    std::optional<double> to;  // 終了時刻（秒単位、ミリ秒まで対応）
    
    // デバッグモード
    bool debug = false;
    
    // ヘルプ表示
    bool help = false;
    
    // バージョン表示
    bool version = false;
};

// コマンドライン引数を解析して Options を返す
Options parse_options(int argc, char* argv[]);

// ヘルプメッセージを表示
void print_help(const char* program_name);

// バージョン情報を表示
void print_version();

#endif // OPTIONS_HPP

