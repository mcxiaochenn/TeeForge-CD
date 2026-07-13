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

static char *dl_url(const char *url, size_t *out) {
    char cmd[1024];
    char *d = NULL; size_t l = 0, c = 65536; char b[4096];
    log_msg(LOG_DEBUG, "下载 [Download]: %s", url);
    d = malloc(c); if (!d) return NULL;
    snprintf(cmd, sizeof(cmd), "curl -sL '%s'", url);
    FILE *fp = popen(cmd, "r"); if (!fp) { free(d); return NULL; }
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
    log_msg(LOG_INFO, "CDN URL: %s", url);

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

    /* 解密 */
    char dec[1024];
    snprintf(dec, sizeof(dec),
        "cat '%s' | base64 -d 2>/dev/null | python3 -c \""
        "import sys,hashlib;"
        "k=hashlib.sha256(b'%s').digest();"
        "d=sys.stdin.buffer.read();"
        "sys.stdout.buffer.write(bytes(b^k[i%%len(k)] for i,b in enumerate(d)))\" > '%s' 2>/dev/null",
        tmp_e, pubkey, tmp_d);
    int r = system(dec); unlink(tmp_e);
    if (r != 0) { log_msg(LOG_ERROR, "解密失败 [Decrypt failed]"); unlink(tmp_d); return -1; }

    size_t dec_len = 0; char *data = read_file(tmp_d, &dec_len); unlink(tmp_d);
    if (!data || dec_len < 100 || !strstr(data, "AndroidAttestation")) {
        log_msg(LOG_ERROR, "无效数据 [Invalid data]"); free(data); return -1; }
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
