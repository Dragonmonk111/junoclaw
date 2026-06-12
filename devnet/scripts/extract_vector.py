import hashlib

with open("/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet/mayo_vector.rs", "r") as f:
    content = f.read()

pk_start = content.find("const PK: &[u8] = &[") + len("const PK: &[u8] = &[")
pk_end = content.find("];", pk_start)
pk_str = content[pk_start:pk_end]
pk_bytes = [int(x.strip()) for x in pk_str.replace("\n", "").replace(" ", "").split(",") if x.strip()]

sig_start = content.find("const SIG: &[u8] = &[") + len("const SIG: &[u8] = &[")
sig_end = content.find("];", sig_start)
sig_str = content[sig_start:sig_end]
sig_bytes = [int(x.strip()) for x in sig_str.replace("\n", "").replace(" ", "").split(",") if x.strip()]

pk_hex = "".join("%02x" % b for b in pk_bytes)
sig_hex = "".join("%02x" % b for b in sig_bytes)

print("PK_HEX_LEN:", len(pk_hex))
print("SIG_HEX_LEN:", len(sig_hex))
print("PK_HASH:", hashlib.sha256(bytes(pk_bytes)).hexdigest())
with open("/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet/mayo_vector.hex", "w") as out:
    out.write("PK_HEX=\n")
    out.write(pk_hex)
    out.write("\nSIG_HEX=\n")
    out.write(sig_hex)
    out.write("\n")
print("written to mayo_vector.hex")
