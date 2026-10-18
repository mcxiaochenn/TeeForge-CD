"""准备三槽位发布数据；全部格式校验成功后才生成发布目录。
Validate all slots before preparing the publish directory.
"""

import argparse
import base64
import codecs
import hashlib
from pathlib import Path
import re
import subprocess
import tempfile

MAX_DOWNLOAD = 2 * 1024 * 1024


def slot_filename(month, slot):
    if not re.fullmatch(r"[0-9]{4}-(0[1-9]|1[0-2])", month) or slot not in (1, 2, 3):
        raise ValueError("月份或槽位无效 [Invalid month or slot]")
    seed = month if slot == 1 else f"{month}_slot_{slot}"
    return hashlib.sha256(seed.encode()).hexdigest()[:12]


def decode_source(data):
    for _ in range(10):
        data = base64.b64decode(b"".join(data.split()), validate=True)
    data = bytes.fromhex(data.decode("ascii"))
    return codecs.decode(data.decode("utf-8"), "rot_13").encode("utf-8")


def wrap(data, public_key):
    key = hashlib.sha256(public_key).digest()
    return base64.b64encode(bytes(b ^ key[i % len(key)] for i, b in enumerate(data)))


def prepare(input_dir, output_dir, public_key, month, validator):
    sources = []
    # 原始材料仅写入权限受限的临时目录，不输出材料或解析器原始诊断。
    # Keep raw material private and suppress parser payload diagnostics.
    with tempfile.TemporaryDirectory(prefix="teeforge-keybox-") as temp:
        for slot in (1, 2, 3):
            path = Path(input_dir) / f"megatron_raw_{slot}"
            with path.open("rb") as stream:
                raw = stream.read(MAX_DOWNLOAD + 1)
            if not raw or len(raw) > MAX_DOWNLOAD:
                raise ValueError(f"槽位 {slot} 大小无效 [Invalid slot size]")
            decoded = Path(temp) / f"slot-{slot}.xml"
            decoded.write_bytes(decode_source(raw))
            checked = subprocess.run(
                [str(validator), "--keybox-validate", str(decoded), "--json"],
                capture_output=True, check=False,
            )
            if checked.returncode:
                raise ValueError(f"槽位 {slot} 格式校验失败 [Slot format validation failed]")
            payload = wrap(raw, public_key)
            if len(payload) > MAX_DOWNLOAD:
                raise ValueError(f"槽位 {slot} 发布数据超限 [Slot payload exceeds limit]")
            sources.append((slot_filename(month, slot), payload))
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    for name, payload in sources:
        (output / name).write_bytes(payload)
    # 保留既有掩护文件，真实槽位使用独立命名空间。
    # Preserve existing cover files; slot names use a separate namespace.
    import os
    for index in range(1, 15):
        name = hashlib.sha256(f"{month}_fake_{index}".encode()).hexdigest()[:12]
        (output / name).write_bytes(wrap(os.urandom(len(raw)), public_key))


def main():
    parser = argparse.ArgumentParser()
    for name in ("input", "output", "public-key", "month", "validator"):
        parser.add_argument(f"--{name}", required=True)
    args = parser.parse_args()
    try:
        public_key = Path(args.public_key).read_bytes().strip()
        if not public_key:
            raise ValueError("公钥为空 [Empty public key]")
        prepare(args.input, args.output, public_key, args.month, args.validator)
    except Exception:
        # 解码异常可能带入材料；统一只输出阶段信息。
        # Decoder errors can contain input; report only the failing stage.
        raise SystemExit("三槽位准备失败，未发布 [Slot preparation failed; nothing published]") from None


if __name__ == "__main__":
    main()
