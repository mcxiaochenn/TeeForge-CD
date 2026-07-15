#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* ===== Keybox Management ===== */

/* ===== 内置解码函数 Built-in decode functions ===== */
/* 替代 system("base64 -d") 和 system("xxd -r -p")，消除临时文件和 fork */
/* Replaces system("base64 -d") and system("xxd -r -p"), eliminates temp files and forks */

/* base64 解码表 Base64 decode table */
static const signed char b64_table[256] = {
    ['A']=0,['B']=1,['C']=2,['D']=3,['E']=4,['F']=5,['G']=6,['H']=7,
    ['I']=8,['J']=9,['K']=10,['L']=11,['M']=12,['N']=13,['O']=14,['P']=15,
    ['Q']=16,['R']=17,['S']=18,['T']=19,['U']=20,['V']=21,['W']=22,['X']=23,
    ['Y']=24,['Z']=25,['a']=26,['b']=27,['c']=28,['d']=29,['e']=30,['f']=31,
    ['g']=32,['h']=33,['i']=34,['j']=35,['k']=36,['l']=37,['m']=38,['n']=39,
    ['o']=40,['p']=41,['q']=42,['r']=43,['s']=44,['t']=45,['u']=46,['v']=47,
    ['w']=48,['x']=49,['y']=50,['z']=51,['0']=52,['1']=53,['2']=54,['3']=55,
    ['4']=56,['5']=57,['6']=58,['7']=59,['8']=60,['9']=61,['+']=62,['/']=63,
};

/*
 * base64_decode — 纯 C base64 解码 Pure C base64 decode
 * 返回 0 成功，-1 失败 Returns 0 on success, -1 on failure
 * 输出由调用方 free Output must be freed by caller
 */
static int base64_decode(const char *in, size_t in_len, char **out, size_t *out_len) {
    if (!in || in_len == 0) return -1;

    /* 计算输出长度（最多 3/4） Calculate output length (max 3/4 of input) */
    size_t max_out = (in_len / 4) * 3 + 3;
    char *buf = malloc(max_out);
    if (!buf) return -1;

    size_t o = 0;
    uint32_t accum = 0;
    int bits = 0;

    for (size_t i = 0; i < in_len; i++) {
        unsigned char c = (unsigned char)in[i];
        if (c == '=' || c == '\n' || c == '\r') continue;  /* 跳过填充和换行 Skip padding and newlines */
        signed char val = b64_table[c];
        if (val < 0) continue;  /* 跳过无效字符 Skip invalid chars */

        accum = (accum << 6) | val;
        bits += 6;
        if (bits >= 8) {
            bits -= 8;
            buf[o++] = (char)((accum >> bits) & 0xFF);
        }
    }

    if (o == 0) { free(buf); return -1; }
    *out = buf;
    if (out_len) *out_len = o;
    return 0;
}

/*
 * hex_decode — 纯 C hex 解码 Pure C hex decode (替代 xxd -r -p)
 * 返回 0 成功，-1 失败 Returns 0 on success, -1 on failure
 * 输出由调用方 free Output must be freed by caller
 */
static int hex_decode(const char *in, size_t in_len, char **out, size_t *out_len) {
    if (!in || in_len < 2) return -1;

    size_t max_out = in_len / 2 + 1;
    char *buf = malloc(max_out);
    if (!buf) return -1;

    size_t o = 0;
    for (size_t i = 0; i + 1 < in_len; i += 2) {
        /* 跳过非 hex 字符（换行等） Skip non-hex chars (newlines etc) */
        char hi = in[i], lo = in[i + 1];
        int h = -1, l = -1;

        if (hi >= '0' && hi <= '9') h = hi - '0';
        else if (hi >= 'a' && hi <= 'f') h = hi - 'a' + 10;
        else if (hi >= 'A' && hi <= 'F') h = hi - 'A' + 10;
        else continue;

        if (lo >= '0' && lo <= '9') l = lo - '0';
        else if (lo >= 'a' && lo <= 'f') l = lo - 'a' + 10;
        else if (lo >= 'A' && lo <= 'F') l = lo - 'A' + 10;
        else continue;

        buf[o++] = (char)((h << 4) | l);
    }

    if (o == 0) { free(buf); return -1; }
    *out = buf;
    if (out_len) *out_len = o;
    return 0;
}

/*
 * rot13_decode — 原地 ROT13 解码 In-place ROT13 decode
 */
static void rot13_decode(char *data, size_t len) {
    for (size_t i = 0; i < len; i++) {
        char c = data[i];
        if (c >= 'A' && c <= 'Z')      data[i] = 'A' + (c - 'A' + 13) % 26;
        else if (c >= 'a' && c <= 'z') data[i] = 'a' + (c - 'a' + 13) % 26;
    }
}

/* ===== 辅助函数 Helper functions ===== */

static int get_month_filename(char *f, size_t sz) {
    f[0] = '\0';
    time_t n = time(NULL);
    struct tm *t = localtime(&n);
    if (!t) return -1;
    char m[16];
    snprintf(m, sizeof(m), "%04d-%02d", t->tm_year + 1900, t->tm_mon + 1);
    char cmd[256];
    snprintf(cmd, sizeof(cmd), "echo -n '%s' | sha256sum | head -c 12", m);
    FILE *fp = popen(cmd, "r");
    if (!fp) return -1;
    if (!fgets(f, sz, fp)) { pclose(fp); return -1; }
    pclose(fp);
    char *nl = strchr(f, '\n'); if (nl) *nl = '\0';
    return f[0] ? 0 : -1;
}

/* 尝试下载工具 Try a download tool, return popen handle */
static FILE *try_download(const char *url, const char *tool) {
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "%s '%s'", tool, url);
    log_msg(LOG_DEBUG, "尝试下载 [Trying]: %s", tool);
    return popen(cmd, "r");
}

/* 尝试下载并读取数据 Try download and read data, NULL if no data */
static char *try_download_and_read(const char *url, const char *tool, size_t *out) {
    FILE *fp = try_download(url, tool);
    if (!fp) return NULL;

    char *d = NULL; size_t l = 0, c = 65536; char b[4096];
    d = malloc(c);
    if (!d) { pclose(fp); return NULL; }

    while (1) {
        size_t n = fread(b, 1, sizeof(b), fp);
        if (n == 0) break;
        if (l+n+1 > c) {
            c *= 2;
            char *nd = realloc(d, c);
            if (!nd) { free(d); pclose(fp); return NULL; }
            d = nd;
        }
        memcpy(d+l, b, n); l += n;
    }
    pclose(fp);

    if (l == 0) { free(d); return NULL; }
    d[l] = '\0';
    if (out) *out = l;
    return d;
}

static char *dl_url(const char *url, size_t *out) {
    log_msg(LOG_DEBUG, "下载 [Download]: %s", url);

    /* 降级策略 Fallback: curl → wget → busybox wget */
    char *data = NULL;
    size_t len = 0;

    data = try_download_and_read(url, "curl -sL", &len);
    if (data) goto done;

    data = try_download_and_read(url, "wget -qO-", &len);
    if (data) goto done;

    if (dir_exists("/data/adb/ksu/bin")) {
        data = try_download_and_read(url, "/data/adb/ksu/bin/busybox wget -qO-", &len);
        if (data) goto done;
    }
    if (dir_exists("/data/adb/ap/bin")) {
        data = try_download_and_read(url, "/data/adb/ap/bin/busybox wget -qO-", &len);
        if (data) goto done;
    }
    if (dir_exists("/data/adb/magisk")) {
        data = try_download_and_read(url, "/data/adb/magisk/busybox wget -qO-", &len);
        if (data) goto done;
    }

    return NULL;

done:
    if (out) *out = len;
    return data;
}

/* ===== 拆分后的子函数 Decomposed sub-functions ===== */

/*
 * keybox_build_urls — 从混淆变量构建 URL 和公钥 Build URLs and pubkey from obfuscated variables
 * 成功返回 0，失败返回 -1 Returns 0 on success, -1 on failure
 */
static int keybox_build_urls(char *cdn, size_t cdn_sz,
                             char *status_url, size_t st_sz,
                             char *pubkey, size_t pk_sz) {
    /* 混淆变量 Obfuscated variables */
    char G[]="aHR0cHM6Ly9jZG"; char M[]="4uanNkZWxpdnIub";
    char T[]="mV0L2doL21jeG"; char A[]="lhb2NoZW5uL1R";
    char P[]="lZUZvcmdlLUNEQG9tZy9rZXlib3gv";
    char F[]="aHR0cHM6Ly9jZG"; char K[]="4uanNkZWxpdnIub";
    char R[]="mV0L2doL21jeG"; char X[]="lhb2NoZW5uL1R";
    char D[]="lZUZvcmdlLUNEQG1haW4va2V5Ym94L2tleS1zdGF0dXM=";
    char J[]="c3NoLWVkMjU1MTkg"; char N[]="QUFBQUMzTnphQzFs";
    char V[]="WkRJMU5URTVBQUFB"; char Z[]="SUU5K1J3NVhadklV";
    char E[]="bW1Wc3pKR0FZRHIwV0krRWp6cHFnSCtVZ0NwL05pZlM=";

    /* 拼合 base64 字符串 Concatenate base64 strings */
    char cdn_b64[256], st_b64[256], pk_b64[256];
    snprintf(cdn_b64, sizeof(cdn_b64), "%s%s%s%s%s", G, M, T, A, P);
    snprintf(st_b64, sizeof(st_b64), "%s%s%s%s%s", F, K, R, X, D);
    snprintf(pk_b64, sizeof(pk_b64), "%s%s%s%s%s", J, N, V, Z, E);

    /* base64 解码 Decode base64 */
    char *cdn_dec = NULL, *st_dec = NULL, *pk_dec = NULL;
    size_t cdn_dec_len = 0, st_dec_len = 0, pk_dec_len = 0;

    if (base64_decode(cdn_b64, strlen(cdn_b64), &cdn_dec, &cdn_dec_len) != 0) {
        log_msg(LOG_ERROR, "CDN URL base64 解码失败 [CDN URL base64 decode failed]");
        return -1;
    }
    if (base64_decode(st_b64, strlen(st_b64), &st_dec, &st_dec_len) != 0) {
        log_msg(LOG_ERROR, "Status URL base64 解码失败 [Status URL base64 decode failed]");
        free(cdn_dec);
        return -1;
    }
    if (base64_decode(pk_b64, strlen(pk_b64), &pk_dec, &pk_dec_len) != 0) {
        log_msg(LOG_ERROR, "Pubkey base64 解码失败 [Pubkey base64 decode failed]");
        free(cdn_dec); free(st_dec);
        return -1;
    }

    /* 复制到输出缓冲区 Copy to output buffers */
    snprintf(cdn, cdn_sz, "%.*s", (int)cdn_dec_len, cdn_dec);
    snprintf(status_url, st_sz, "%.*s", (int)st_dec_len, st_dec);
    snprintf(pubkey, pk_sz, "%.*s", (int)pk_dec_len, pk_dec);

    free(cdn_dec); free(st_dec); free(pk_dec);
    return 0;
}

/*
 * keybox_compute_sha256 — 通过 sha256sum 计算哈希 Compute SHA256 via sha256sum
 * 成功返回 0，失败返回 -1 Returns 0 on success, -1 on failure
 */
static int keybox_compute_sha256(const char *input, size_t len, unsigned char *output) {
    char tmp_in[256], tmp_out[256];
    snprintf(tmp_in, sizeof(tmp_in), "/data/local/tmp/.pk_%d", getpid());
    snprintf(tmp_out, sizeof(tmp_out), "/data/local/tmp/.hash_%d", getpid());

    /* 写入输入 Write input */
    FILE *fp = fopen(tmp_in, "w");
    if (!fp) return -1;
    fwrite(input, 1, len, fp);
    fclose(fp);

    /* sha256sum（toybox 自带，无需 openssl） */
    char cmd[512];
    snprintf(cmd, sizeof(cmd),
        "sha256sum '%s' | awk '{print $1}' > '%s' 2>/dev/null",
        tmp_in, tmp_out);
    int ret = system(cmd);
    unlink(tmp_in);

    if (ret != 0) {
        log_msg(LOG_ERROR, "sha256sum 执行失败 [sha256sum execution failed]");
        unlink(tmp_out);
        return -1;
    }

    /* 读取 hex 结果 Read hex result */
    FILE *fh = fopen(tmp_out, "r");
    if (!fh) { unlink(tmp_out); return -1; }

    char hex[65] = {0};
    if (!fgets(hex, sizeof(hex), fh)) {
        fclose(fh); unlink(tmp_out); return -1;
    }
    fclose(fh);
    unlink(tmp_out);

    /* 解析 hex Parse hex */
    for (int i = 0; i < 32 && hex[i * 2]; i++) {
        sscanf(hex + i * 2, "%2hhx", &output[i]);
    }
    return 0;
}

/*
 * keybox_decrypt — 完整解密流程 Full decryption pipeline
 * 输入：下载的加密数据 Input: downloaded encrypted data
 * 输出：解密后的 XML Output: decrypted XML
 * 成功返回 0，失败返回 -1 Returns 0 on success, -1 on failure
 * *out_data 由调用方 free *out_data must be freed by caller
 */
static int keybox_decrypt(const char *enc_data, size_t enc_len,
                          const char *pubkey, char **out_data, size_t *out_len) {
    /* 步骤 1：首次 base64 解码 Step 1: First base64 decode */
    char *raw = NULL;
    size_t raw_len = 0;
    if (base64_decode(enc_data, enc_len, &raw, &raw_len) != 0) {
        log_msg(LOG_ERROR, "首次 base64 解码失败 [First base64 decode failed]");
        return -1;
    }
    log_msg(LOG_DEBUG, "首次 base64 解码 [First base64 decode]: %zu bytes", raw_len);

    /* 步骤 2：计算 SHA256 密钥 Step 2: Compute SHA256 key */
    unsigned char key[32] = {0};
    if (keybox_compute_sha256(pubkey, strlen(pubkey), key) != 0) {
        free(raw);
        return -1;
    }

    char key_hex[65] = {0};
    for (int i = 0; i < 32; i++) snprintf(key_hex + i * 2, 3, "%02x", key[i]);
    log_msg(LOG_DEBUG, "SHA256 key: %s", key_hex);

    /* 步骤 3：XOR 解密 Step 3: XOR decrypt */
    char *xor_result = malloc(raw_len + 1);
    if (!xor_result) { free(raw); return -1; }
    for (size_t i = 0; i < raw_len; i++) {
        xor_result[i] = raw[i] ^ key[i % 32];
    }
    xor_result[raw_len] = '\0';
    free(raw);

    log_msg(LOG_DEBUG, "XOR 结果前 40 字节 [XOR result first 40 bytes]: %.*s",
            (int)(raw_len > 40 ? 40 : raw_len), xor_result);

    /*
     * 上游 Megatron 解密流程（参考 Integrity-Box key.sh）：
     * Upstream Megatron decryption (reference: Integrity-Box key.sh):
     * 1. base64 解码 10 次 [10x base64 decode]
     * 2. hex 解码 [hex decode]
     * 3. ROT13 解码 [ROT13 decode]
     */

    /* 步骤 4：base64 解码 10 次 Step 4: 10x base64 decode (纯 C，无临时文件) */
    char *cur_data = xor_result;
    size_t cur_len = raw_len;

    for (int i = 0; i < 10; i++) {
        char *decoded = NULL;
        size_t decoded_len = 0;

        if (base64_decode(cur_data, cur_len, &decoded, &decoded_len) != 0) {
            log_msg(LOG_ERROR, "base64 解码失败于第 %d 次 [base64 decode failed at iteration %d]", i + 1, i + 1);
            free(cur_data);
            return -1;
        }

        log_msg(LOG_DEBUG, "  base64 第 %d 次 [base64 #%d]: %zu → %zu bytes",
                i + 1, i + 1, cur_len, decoded_len);

        free(cur_data);
        cur_data = decoded;
        cur_len = decoded_len;
    }

    /* 步骤 5：hex 解码 Step 5: hex decode (纯 C，无临时文件) */
    char *hex_decoded = NULL;
    size_t hex_len = 0;
    if (hex_decode(cur_data, cur_len, &hex_decoded, &hex_len) != 0) {
        log_msg(LOG_ERROR, "hex 解码失败 [Hex decode failed]");
        free(cur_data);
        return -1;
    }
    free(cur_data);
    log_msg(LOG_DEBUG, "hex 解码 [Hex decode]: %zu bytes", hex_len);

    /* 步骤 6：ROT13 解码 Step 6: ROT13 decode */
    rot13_decode(hex_decoded, hex_len);
    log_msg(LOG_DEBUG, "ROT13 解码 [ROT13 decode]: %zu bytes", hex_len);

    *out_data = hex_decoded;
    *out_len = hex_len;
    return 0;
}

/*
 * keybox_write — 验证并写入 keybox 文件 Validate and write keybox file
 * 成功返回 0，失败返回 -1 Returns 0 on success, -1 on failure
 */
static int keybox_write(const char *data, size_t len) {
    /* 验证 Verify */
    if (len < 100 || !strstr(data, "AndroidAttestation")) {
        log_msg(LOG_ERROR, "无效数据 [Invalid data]: 缺少 AndroidAttestation 标记");
        return -1;
    }
    log_msg(LOG_INFO, "解密成功 [Decrypted]: %zu bytes", len);

    /* 写入 keybox 目录 Write to keybox dir */
    char kp[512];
    snprintf(kp, sizeof(kp), "%s/keybox.xml", g_config.keybox_dir);
    ensure_dir(g_config.keybox_dir);

    if (file_exists(kp)) {
        char bak[512];
        snprintf(bak, sizeof(bak), "%s.bak", kp);
        rename(kp, bak);
    }

    if (write_file(kp, data, len) != 0) return -1;
    log_msg(LOG_INFO, "已更新 [Updated]: %s", kp);

    /* 同步到 Tricky Store Sync to Tricky Store */
    char ts[512];
    snprintf(ts, sizeof(ts), "/data/adb/tricky_store/keybox.xml");
    if (write_file(ts, data, len) == 0) {
        log_msg(LOG_INFO, "已同步 Tricky Store [Synced]");
    }

    return 0;
}

/* ===== 主入口 Main entry ===== */

int keybox_fetch(void) {
    log_msg(LOG_INFO, "开始获取 keybox [Starting keybox fetch]...");

    /* 生成文件名 Generate filename */
    char fn[32];
    if (get_month_filename(fn, sizeof(fn)) != 0) {
        log_msg(LOG_ERROR, "无法生成文件名 [Failed to generate filename]");
        return -1;
    }
    log_msg(LOG_DEBUG, "文件名 [Filename]: %s", fn);

    /* 构建 URL 和公钥 Build URLs and pubkey */
    char cdn[256] = "", status_url[256] = "", pubkey[256] = "";
    if (keybox_build_urls(cdn, sizeof(cdn), status_url, sizeof(status_url),
                          pubkey, sizeof(pubkey)) != 0) {
        return -1;
    }

    char url[512];
    snprintf(url, sizeof(url), "%s%s", cdn, fn);
    log_msg(LOG_INFO, "CDN URL: [hidden]");
    log_msg(LOG_DEBUG, "Pubkey 长度 [Pubkey length]: %zu", strlen(pubkey));

    /* 下载 Download */
    size_t enc_len = 0;
    char *enc = dl_url(url, &enc_len);
    if (!enc) {
        log_msg(LOG_ERROR, "下载失败 [Download failed]");
        return -1;
    }
    log_msg(LOG_INFO, "下载成功 [Downloaded]: %zu bytes", enc_len);

    /* 解密 Decrypt */
    char *data = NULL;
    size_t data_len = 0;
    int ret = keybox_decrypt(enc, enc_len, pubkey, &data, &data_len);
    free(enc);

    if (ret != 0) return -1;

    /* 写入 Write */
    ret = keybox_write(data, data_len);
    free(data);
    return ret;
}
