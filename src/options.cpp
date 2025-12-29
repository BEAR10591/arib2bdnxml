#include "options.hpp"
#include <iostream>
#include <sstream>
#include <cstring>
#include <algorithm>

// 除外する libaribcaption オプション
static const std::vector<std::string> EXCLUDED_OPTS = {
    "sub_type",
    "ass_single_rect",
    "canvas_size"
};

static bool is_excluded_opt(const std::string& key) {
    return std::find(EXCLUDED_OPTS.begin(), EXCLUDED_OPTS.end(), key) != EXCLUDED_OPTS.end();
}

static void parse_libaribcaption_opts(const std::string& opts_str, std::map<std::string, std::string>& result) {
    std::string remaining = opts_str;
    
    while (!remaining.empty()) {
        // 前後の空白を削除
        remaining.erase(0, remaining.find_first_not_of(" \t"));
        if (remaining.empty()) break;
        
        // 最初の '=' の位置を探す（これが key=value の区切り）
        size_t eq_pos = remaining.find('=');
        if (eq_pos == std::string::npos) {
            std::cerr << "警告: libaribcaption オプション '" << remaining << "' は key=value 形式ではありません。スキップします。" << std::endl;
            break;
        }
        
        // key を取得
        std::string key = remaining.substr(0, eq_pos);
        key.erase(0, key.find_first_not_of(" \t"));
        key.erase(key.find_last_not_of(" \t") + 1);
        
        // value の開始位置
        size_t value_start = eq_pos + 1;
        
        // value を取得（次の key= が見つかるまで、または文字列の終わりまで）
        // ただし、クォートで囲まれている場合は、そのクォート内のカンマは無視
        std::string value;
        bool in_quotes = false;
        char quote_char = '\0';
        size_t i = value_start;
        
        while (i < remaining.length()) {
            char c = remaining[i];
            
            // クォートの開始/終了を検出
            if ((c == '"' || c == '\'') && (i == value_start || remaining[i-1] != '\\')) {
                if (!in_quotes) {
                    in_quotes = true;
                    quote_char = c;
                } else if (c == quote_char) {
                    in_quotes = false;
                    quote_char = '\0';
                }
                value += c;
                i++;
                continue;
            }
            
            // クォート内でない場合、カンマで次のオプションが始まる可能性をチェック
            if (c == ',' && !in_quotes) {
                // 次の文字列が key=value 形式かチェック
                size_t next_start = i + 1;
                while (next_start < remaining.length() && (remaining[next_start] == ' ' || remaining[next_start] == '\t')) {
                    next_start++;
                }
                // 次の '=' の位置を探す（カンマの後から）
                size_t next_eq = remaining.find('=', next_start);
                if (next_eq != std::string::npos) {
                    // カンマと '=' の間に有効な文字（key）があるかチェック
                    std::string potential_key = remaining.substr(next_start, next_eq - next_start);
                    potential_key.erase(0, potential_key.find_first_not_of(" \t"));
                    potential_key.erase(potential_key.find_last_not_of(" \t") + 1);
                    // keyが空でなく、カンマを含まない場合は次のオプションと判断
                    if (!potential_key.empty() && potential_key.find(',') == std::string::npos) {
                        // 次の key=value が見つかったので、ここで終了
                        break;
                    }
                }
            }
            
            value += c;
            i++;
        }
        
        // value の前後の空白を削除
        value.erase(0, value.find_first_not_of(" \t"));
        value.erase(value.find_last_not_of(" \t") + 1);
        
        // クォートを削除（valueの前後）
        if (value.length() >= 2) {
            if ((value[0] == '"' && value[value.length()-1] == '"') ||
                (value[0] == '\'' && value[value.length()-1] == '\'')) {
                value = value.substr(1, value.length() - 2);
            }
        }
        
        // 除外オプションをチェック
        if (is_excluded_opt(key)) {
            std::cerr << "警告: libaribcaption オプション '" << key << "' は本ツールでは使用できません。スキップします。" << std::endl;
        } else {
            result[key] = value;
            // デバッグ出力（開発時のみ）
            // std::cout << "DEBUG: オプション '" << key << "' = '" << value << "'" << std::endl;
        }
        
        // 次のオプションへ進む
        if (i < remaining.length() && remaining[i] == ',') {
            remaining = remaining.substr(i + 1);
        } else {
            remaining = remaining.substr(i);
        }
    }
}

Options parse_options(int argc, char* argv[]) {
    Options opts;
    
    if (argc < 2) {
        print_help(argv[0]);
        std::exit(1);
    }
    
    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        
        if (arg == "--help" || arg == "-h") {
            opts.help = true;
            print_help(argv[0]);
            std::exit(0);
        } else if (arg == "--version" || arg == "-v") {
            opts.version = true;
            print_version();
            std::exit(0);
        } else if (arg == "--libaribcaption-opt" && i + 1 < argc) {
            parse_libaribcaption_opts(argv[++i], opts.libaribcaption_opts);
        } else if (arg == "--output" && i + 1 < argc) {
            opts.output_dir = argv[++i];
        } else if ((arg == "--resolution" || arg == "-r") && i + 1 < argc) {
            opts.resolution = argv[++i];
        } else if (arg == "--debug") {
            opts.debug = true;
        } else if (arg[0] != '-') {
            // 入力ファイル
            if (opts.input_file.empty()) {
                opts.input_file = arg;
            } else {
                std::cerr << "エラー: 複数の入力ファイルが指定されています。" << std::endl;
                std::exit(1);
            }
        } else {
            std::cerr << "エラー: 不明なオプション '" << arg << "'" << std::endl;
            print_help(argv[0]);
            std::exit(1);
        }
    }
    
    if (opts.input_file.empty()) {
        std::cerr << "エラー: 入力ファイルが指定されていません。" << std::endl;
        print_help(argv[0]);
        std::exit(1);
    }
    
    return opts;
}

void print_help(const char* program_name) {
    std::cout << "使用方法: " << program_name << " [オプション] <入力ファイル>\n\n";
    std::cout << "オプション:\n";
    std::cout << "  --resolution, -r <解像度> 出力解像度（1920x1080, 1440x1080, 1280x720, 720x480）\n";
    std::cout << "                            指定がない場合は動画解像度に基づいて自動決定\n";
    std::cout << "  --libaribcaption-opt <オプション>\n";
    std::cout << "                            libaribcaption オプション（key=value,key=value 形式）\n";
    std::cout << "                            除外: sub_type, ass_single_rect, canvas_size\n";
    std::cout << "  --output <ディレクトリ>   出力ディレクトリ\n";
    std::cout << "  --debug                   デバッグログを出力\n";
    std::cout << "  --help, -h                このヘルプを表示\n";
    std::cout << "  --version, -v             バージョン情報を表示\n";
}

void print_version() {
    std::cout << "arib2bdnxml 0.1.0\n";
}

