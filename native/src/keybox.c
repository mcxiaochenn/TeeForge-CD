#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>
#include <unistd.h>

/* ===== Keybox Management ===== */

static void get_month_filename(char *f, size_t sz) {
    time_t n = time(NULL);
    struct tm *t = localtime(&n);
    char m[16];
    snprintf(m, sizeof(m), "%04d-%02d", t->tm_year + 1900, t->tm_mon + 1);
    char cmd[256];
    snprintf(cmd, sizeof(cmd), "echo -n '%s' | sha256sum | head -c 12", m);
    FILE *fp = popen(cmd, "r");
    if (fp) { if (fgets(f, sz, fp)) { char *nl = strchr(f, '\n'); if (nl) *nl = '\0'; } pclose(fp); }
}

/* 尝试下载，返回 popen 句柄 Try download, return popen handle */
static FILE *try_download(const char *url, const char *tool) {
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "%s '%s'", tool, url);
    log_msg(LOG_DEBUG, "尝试下载 [Trying]: %s", tool);
    return popen(cmd, "r");
}

static char *dl_url(const char *url, size_t *out) {
    char *d = NULL; size_t l = 0, c = 65536; char b[4096];
    log_msg(LOG_DEBUG, "下载 [Download]: %s", url);
    d = malloc(c); if (!d) return NULL;

    /* 降级策略 Fallback: curl → wget → busybox wget */
    FILE *fp = NULL;
    fp = try_download(url, "curl -sL");
    if (!fp) fp = try_download(url, "wget -qO-");
    if (!fp && dir_exists("/data/adb/ksu/bin"))
        fp = try_download(url, "/data/adb/ksu/bin/busybox wget -qO-");
    if (!fp && dir_exists("/data/adb/ap/bin"))
        fp = try_download(url, "/data/adb/ap/bin/busybox wget -qO-");
    if (!fp && dir_exists("/data/adb/magisk"))
        fp = try_download(url, "/data/adb/magisk/busybox wget -qO-");
    if (!fp) { free(d); return NULL; }

    while (1) { size_t n = fread(b, 1, sizeof(b), fp); if (n == 0) break;
        if (l+n > c) { c *= 2; char *nd = realloc(d, c); if (!nd) { free(d); pclose(fp); return NULL; } d = nd; }
        memcpy(d+l, b, n); l += n; }
    pclose(fp); if (l == 0) { free(d); return NULL; }
    d[l] = '\0'; if (out) *out = l; return d;
}

int keybox_fetch(void) {
    log_msg(LOG_INFO, "开始获取 keybox [Starting keybox fetch]...");

    char fn[32]; get_month_filename(fn, sizeof(fn));
    log_msg(LOG_DEBUG, "文件名 [Filename]: %s", fn);

    /* ===== 混淆变量 Obfuscated variables ===== */
    char G[]="aHR0cHM6Ly9jZG"; char M[]="4uanNkZWxpdnIub";
    char T[]="mV0L2doL21jeG"; char A[]="lhb2NoZW5uL1R";
    char P[]="lZUZvcmdlLUNEQG9tZy9rZXlib3gv";
    char F[]="aHR0cHM6Ly9jZG"; char K[]="4uanNkZWxpdnIub";
    char R[]="mV0L2doL21jeG"; char X[]="lhb2NoZW5uL1R";
    char D[]="lZUZvcmdlLUNEQG1haW4va2V5Ym94L2tleS1zdGF0dXM=";
    char J[]="c3NoLWVkMjU1MTkg"; char N[]="QUFBQUMzTnphQzFs";
    char V[]="WkRJMU5URTVBQUFB"; char Z[]="SUU5K1J3NVhadklV";
    char E[]="bW1Wc3pKR0FZRHIwV0krRWp6cHFnSCtVZ0NwL05pZlM=";

    /* 拼合 CDN URL */
    char cdn_b64[256]; snprintf(cdn_b64, sizeof(cdn_b64), "%s%s%s%s%s", G,M,T,A,P);
    /* 拼合 STATUS URL */
    char st_b64[256]; snprintf(st_b64, sizeof(st_b64), "%s%s%s%s%s", F,K,R,X,D);
    /* 拼合 PUBKEY */
    char pk_b64[256]; snprintf(pk_b64, sizeof(pk_b64), "%s%s%s%s%s", J,N,V,Z,E);

    /* base64 解码得到原始 URL */
    char cdn_raw[512], st_raw[512], pk_raw[256];
    snprintf(cdn_raw, sizeof(cdn_raw), "echo -n '%s' | base64 -d 2>/dev/null", cdn_b64);
    snprintf(st_raw, sizeof(st_raw), "echo -n '%s' | base64 -d 2>/dev/null", st_b64);
    snprintf(pk_raw, sizeof(pk_raw), "echo -n '%s' | base64 -d 2>/dev/null", pk_b64);

    FILE *fp;
    char cdn[256]="", status_url[256]="", pubkey[256]="";
    fp=popen(cdn_raw,"r"); if(fp){fgets(cdn,sizeof(cdn),fp);char*nl=strchr(cdn,'\n');if(nl)*nl='\0';pclose(fp);}
    fp=popen(st_raw,"r"); if(fp){fgets(status_url,sizeof(status_url),fp);char*nl=strchr(status_url,'\n');if(nl)*nl='\0';pclose(fp);}
    fp=popen(pk_raw,"r"); if(fp){fgets(pubkey,sizeof(pubkey),fp);char*nl=strchr(pubkey,'\n');if(nl)*nl='\0';pclose(fp);}

    /* 构建下载 URL */
    char url[512]; snprintf(url, sizeof(url), "%s%s", cdn, fn);
    log_msg(LOG_INFO, "CDN URL: [hidden]");
    log_msg(LOG_DEBUG, "Pubkey 长度 [Pubkey length]: %zu", strlen(pubkey));

    /* 下载 */
    size_t enc_len = 0;
    char *enc = dl_url(url, &enc_len);
    if (!enc) { log_msg(LOG_ERROR, "下载失败 [Download failed]"); return -1; }
    log_msg(LOG_INFO, "下载成功 [Downloaded]: %zu bytes", enc_len);

    /* 写入临时文件 */
    char tmp_e[256], tmp_d[256];
    snprintf(tmp_e, sizeof(tmp_e), "/data/local/tmp/.kbx_%d", getpid());
    snprintf(tmp_d, sizeof(tmp_d), "/data/local/tmp/.kb_%d", getpid());
    if (write_file(tmp_e, enc, enc_len) != 0) { free(enc); return -1; }
    free(enc);

    /* base64 解码 */
    char tmp_b64[256];
    snprintf(tmp_b64, sizeof(tmp_b64), "/data/local/tmp/.kbx_b64_%d", getpid());
    char b64_cmd[512];
    snprintf(b64_cmd, sizeof(b64_cmd), "base64 -d '%s' > '%s' 2>/dev/null", tmp_e, tmp_b64);
    int r = system(b64_cmd);
    unlink(tmp_e);
    if (r != 0) { log_msg(LOG_ERROR, "base64 解码失败 [base64 decode failed]"); unlink(tmp_b64); return -1; }

    /* 读取解码后的数据 */
    size_t raw_len = 0;
    char *raw = read_file(tmp_b64, &raw_len);
    unlink(tmp_b64);
    if (!raw || raw_len == 0) { log_msg(LOG_ERROR, "解码数据为空 [Decoded data empty]"); free(raw); return -1; }
    log_msg(LOG_DEBUG, "首次 base64 解码 [First base64 decode]: %zu bytes", raw_len);

    /* 计算噪声密钥（sha256sum，兼容无 openssl 的设备） */
    /* Compute noise key via sha256sum (works without openssl) */
    unsigned char key[32] = {0};
    char tmp_pk[256], tmp_hash[256];
    snprintf(tmp_pk, sizeof(tmp_pk), "/data/local/tmp/.pk_%d", getpid());
    snprintf(tmp_hash, sizeof(tmp_hash), "/data/local/tmp/.hash_%d", getpid());

    /* 写入公钥 Write pubkey to temp file */
    FILE *fpk = fopen(tmp_pk, "w");
    if (fpk) { fprintf(fpk, "%s", pubkey); fclose(fpk); }

    /* sha256sum（toybox 自带，无需 openssl） */
    char sha_cmd[512];
    snprintf(sha_cmd, sizeof(sha_cmd),
        "sha256sum '%s' | awk '{print $1}' > '%s' 2>/dev/null",
        tmp_pk, tmp_hash);
    system(sha_cmd);
    unlink(tmp_pk);

    /* 读取 hash hex Read hash hex */
    FILE *fh = fopen(tmp_hash, "r");
    if (fh) {
        char hex[65] = {0};
        if (fgets(hex, sizeof(hex), fh)) {
            for (int i = 0; i < 32 && hex[i*2]; i++) {
                sscanf(hex + i*2, "%2hhx", &key[i]);
            }
        }
        fclose(fh);
    }
    unlink(tmp_hash);

    /* 输出密钥 hash 用于调试 Output key hash for debugging */
    char key_hex[65] = {0};
    for (int i = 0; i < 32; i++) snprintf(key_hex + i*2, 3, "%02x", key[i]);
    log_msg(LOG_DEBUG, "SHA256 key: %s", key_hex);

    /* XOR 解密 */
    char *xor_result = malloc(raw_len + 1);
    if (!xor_result) { free(raw); return -1; }
    for (size_t i = 0; i < raw_len; i++) {
        xor_result[i] = raw[i] ^ key[i % 32];
    }
    xor_result[raw_len] = '\0';
    free(raw);

    /*
     * 上游 Megatron 解密流程（参考 Integrity-Box key.sh）：
     * Upstream Megatron decryption (reference: Integrity-Box key.sh):
     * 1. base64 解码 10 次 [10x base64 decode]
     * 2. hex 解码 [hex decode (xxd -r -p)]
     * 3. ROT13 解码 [ROT13 decode]
     */
    log_msg(LOG_DEBUG, "XOR 结果前 40 字节 [XOR result first 40 bytes]: %.*s",
            (int)(raw_len > 40 ? 40 : raw_len), xor_result);

    char *cur_data = xor_result;
    size_t cur_len = raw_len;
    char tmp_in[256], tmp_out[256];

    /* 步骤1：base64 解码 10 次 Step 1: 10x base64 decode */
    for (int i = 0; i < 10; i++) {
        snprintf(tmp_in, sizeof(tmp_in), "/data/local/tmp/.kbx_b%d_%d", i, getpid());
        snprintf(tmp_out, sizeof(tmp_out), "/data/local/tmp/.kbx_b%d_%d", i + 1, getpid());

        if (write_file(tmp_in, cur_data, cur_len) != 0) break;

        char cmd[512];
        snprintf(cmd, sizeof(cmd), "base64 -d '%s' > '%s' 2>/dev/null", tmp_in, tmp_out);
        int ret = system(cmd);
        unlink(tmp_in);

        if (ret != 0) {
            log_msg(LOG_ERROR, "base64 解码失败于第 %d 次 [base64 decode failed at iteration %d]", i + 1, i + 1);
            unlink(tmp_out);
            free(cur_data);
            return -1;
        }

        size_t out_len = 0;
        char *out_data = read_file(tmp_out, &out_len);
        unlink(tmp_out);

        if (!out_data || out_len == 0) {
            log_msg(LOG_ERROR, "base64 解码结果为空 [base64 decode empty at iteration %d]", i + 1);
            free(out_data);
            free(cur_data);
            return -1;
        }

        log_msg(LOG_DEBUG, "  base64 第 %d 次 [base64 #%d]: %zu → %zu bytes", i + 1, i + 1, cur_len, out_len);

        if (i > 0) free(cur_data);
        cur_data = out_data;
        cur_len = out_len;
    }

    /* 步骤2：hex 解码 Step 2: hex decode (xxd -r -p) */
    snprintf(tmp_in, sizeof(tmp_in), "/data/local/tmp/.kbx_hex_%d", getpid());
    snprintf(tmp_out, sizeof(tmp_out), "/data/local/tmp/.kbx_hexout_%d", getpid());

    if (write_file(tmp_in, cur_data, cur_len) != 0) {
        free(cur_data);
        return -1;
    }
    free(cur_data);

    char hex_cmd[512];
    snprintf(hex_cmd, sizeof(hex_cmd), "xxd -r -p '%s' > '%s' 2>/dev/null", tmp_in, tmp_out);
    int hex_ret = system(hex_cmd);
    unlink(tmp_in);

    if (hex_ret != 0) {
        log_msg(LOG_ERROR, "hex 解码失败 [Hex decode failed]");
        unlink(tmp_out);
        return -1;
    }

    size_t hex_len = 0;
    char *hex_data = read_file(tmp_out, &hex_len);
    unlink(tmp_out);

    if (!hex_data || hex_len == 0) {
        log_msg(LOG_ERROR, "hex 解码结果为空 [Hex decode empty]");
        free(hex_data);
        return -1;
    }
    log_msg(LOG_DEBUG, "hex 解码 [Hex decode]: %zu bytes", hex_len);

    /* 步骤3：ROT13 解码 Step 3: ROT13 decode */
    char *data = malloc(hex_len + 1);
    if (!data) { free(hex_data); return -1; }

    for (size_t i = 0; i < hex_len; i++) {
        char c = hex_data[i];
        if (c >= 'A' && c <= 'Z')      data[i] = 'A' + (c - 'A' + 13) % 26;
        else if (c >= 'a' && c <= 'z') data[i] = 'a' + (c - 'a' + 13) % 26;
        else                           data[i] = c;
    }
    data[hex_len] = '\0';
    free(hex_data);

    size_t dec_len = hex_len;
    log_msg(LOG_DEBUG, "ROT13 解码 [ROT13 decode]: %zu bytes", dec_len);

    /* 验证结果 Verify result */
    if (dec_len < 100 || !strstr(data, "AndroidAttestation")) {
        log_msg(LOG_ERROR, "无效数据 [Invalid data]: 缺少 AndroidAttestation 标记");
        free(data);
        return -1;
    }
    log_msg(LOG_INFO, "解密成功 [Decrypted]: %zu bytes", dec_len);

    /* 写入 */
    char kp[512]; snprintf(kp, sizeof(kp), "%s/keybox.xml", g_config.keybox_dir);
    ensure_dir(g_config.keybox_dir);
    if (file_exists(kp)) { char bak[512]; snprintf(bak,sizeof(bak),"%s.bak",kp); rename(kp,bak); }
    if (write_file(kp, data, dec_len) != 0) { free(data); return -1; }
    log_msg(LOG_INFO, "已更新 [Updated]: %s", kp);

    char ts[512]; snprintf(ts, sizeof(ts), "/data/adb/tricky_store/keybox.xml");
    if (write_file(ts, data, dec_len) == 0) log_msg(LOG_INFO, "已同步 Tricky Store [Synced]");

    free(data); return 0;
}
