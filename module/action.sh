#!/system/bin/sh
# 必须保持 LF 换行，供 Android sh 直接执行 Must remain LF-only
# TeeForge-CD Action — 音量键两级菜单 [Two-level volume-key menu]
#
# 按键约定 [Keys, TWRP style]
#   音量下 = 切换高亮项 / 音量上 = 确认当前项 / 15 秒无操作 = 取消
#   确认前不写入任何文件；取消直接退出且退出码为 0。
#
# 日志 [Logging]
#   全过程同时写入 $LOG_DIR/action-*.log，避免管理器面板截断后无据可查。

MODDIR=${0%/*}
DATA_DIR=/data/adb/teeforge
CONFIG="$DATA_DIR/config.conf"
TF="$MODDIR/teeforge"
LOG_DIR="$DATA_DIR/logs"

mkdir -p "$LOG_DIR" 2>/dev/null
LOG_FILE="$LOG_DIR/action-$(date +%Y%m%d-%H%M%S).log"
TMP_OUT="$LOG_DIR/.action.out.$$"

say() {
    echo "$*"
    echo "$*" >>"$LOG_FILE" 2>/dev/null
}

on_exit() {
    rc=$?
    echo "ACTION_END rc=$rc at $(date +%H:%M:%S)" >>"$LOG_FILE" 2>/dev/null
    rm -f "$TMP_OUT" 2>/dev/null
}
trap on_exit EXIT INT TERM HUP

# 老 shell 可能不支持 pipefail；不支持时退回"先落临时文件再回放"，
# 虽然失去实时输出，但能保住真实退出码。
PIPEFAIL=0
if (set -o pipefail) 2>/dev/null; then
    set -o pipefail
    PIPEFAIL=1
fi

run_logged() {
    if [ "$PIPEFAIL" = "1" ]; then
        "$@" 2>&1 | tee -a "$LOG_FILE"
        return $?
    fi
    "$@" >"$TMP_OUT" 2>&1
    rc=$?
    cat "$TMP_OUT" 2>/dev/null | tee -a "$LOG_FILE"
    return "$rc"
}

cleanup_logs() {
    count=0
    for file in $(ls -1t "$LOG_DIR"/action-*.log 2>/dev/null); do
        count=$((count + 1))
        if [ "$count" -gt 10 ]; then
            rm -f "$file" 2>/dev/null
        fi
    done
}

# 返回值：0 = 音量下（切换）、1 = 音量上（确认）、2 = 超时、3 = 读取失败
read_key() {
    value=$("$TF" --volume 15 --no-rootdetect 2>/dev/null)
    code=$?
    case "$value" in
        0) return 0 ;;
        1) return 1 ;;
        -1) return 2 ;;
    esac
    if [ "$code" -ne 0 ]; then
        return 3
    fi
    return 3
}

render_menu() {
    n=1
    for option in "$@"; do
        if [ "$n" -eq "$index" ]; then
            say "  > $n) $option"
        else
            say "    $n) $option"
        fi
        n=$((n + 1))
    done
}

# pick <标题> <选项...>；成功时设置 PICK_INDEX
pick() {
    title=$1
    shift
    count=$#
    index=1
    say ""
    say "$title"
    while :; do
        render_menu "$@"
        read_key
        state=$?
        case "$state" in
            0)
                index=$((index % count + 1))
                say ""
                ;;
            1)
                PICK_INDEX=$index
                say "  已确认第 $index 项 [confirmed]"
                return 0
                ;;
            2)
                say "  超时未操作，按取消处理 [timeout: cancelled]"
                return 1
                ;;
            *)
                say "  无法读取音量键，请改用 WebUI 或 CLI [volume keys unavailable]"
                return 2
                ;;
        esac
    done
}

cancelled() {
    say ""
    say "已取消，未做任何修改 [Cancelled; nothing changed]"
    exit 0
}

cleanup_logs

RESULT=1
say "========================================"
say " TeeForge-CD Action"
say " 日志 Log: $LOG_FILE"
say "========================================"
say ""
say "音量下 = 切换选项，音量上 = 确认，15 秒无操作视为取消。"
say "Vol- moves the highlight, Vol+ confirms; 15s idle cancels."

if ! pick "是否执行 TeeForge 操作？ [Run TeeForge actions?]" "执行 [Run]" "取消 [Cancel]"; then
    cancelled
fi
if [ "$PICK_INDEX" -eq 2 ]; then
    cancelled
fi

if ! pick "选择操作 [Choose an action]" "更新 Keybox [Update keybox]" "更新目标作用域 [Update target scope]"; then
    cancelled
fi

case "$PICK_INDEX" in
    1)
        if ! pick "Keybox 来源 [Keybox source]" "本机 RKP 签发并应用 [Issue with device RKP]" "远程公共密钥池 [Shared pool from CDN]"; then
            cancelled
        fi
        case "$PICK_INDEX" in
            1)
                say ""
                say "[1/2] 本机 RKP 签发 [Issuing on device]..."
                run_logged "$TF" --config "$CONFIG" --rkp-issue
                RESULT=$?
                if [ "$RESULT" -eq 0 ]; then
                    say ""
                    say "[2/2] 安装 RKP 产物到后端 [Installing artifacts]..."
                    run_logged "$TF" --config "$CONFIG" --keybox --source rkp
                    RESULT=$?
                else
                    say "RKP 签发失败，未改动后端 keybox [issuance failed; backends untouched]"
                fi
                ;;
            2)
                say ""
                say "从远程密钥池获取 [Fetching shared keybox]..."
                run_logged "$TF" --config "$CONFIG" --keybox --source remote
                RESULT=$?
                ;;
        esac
        ;;
    2)
        say ""
        say "更新目标作用域 [Updating target scope]..."
        run_logged "$TF" --config "$CONFIG" --generate
        RESULT=$?
        ;;
esac

if [ "$RESULT" -eq 0 ]; then
    # 描述更新失败不影响主流程结果，但仍写进日志。
    "$TF" --config "$CONFIG" --update-desc >/dev/null 2>&1
fi

say ""
say "========================================"
if [ "$RESULT" -eq 0 ]; then
    say " 完成 [Done] - 无需重启 [No reboot needed]"
else
    say " 失败 [Failed] - 退出码 [exit] $RESULT"
fi
say " 日志 [Log]: $LOG_FILE"
say "========================================"

exit "$RESULT"
