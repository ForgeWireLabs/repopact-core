import json, math, statistics
from pathlib import Path
base_path=Path(r"C:\Projects\repopact-core-wi083-ac7-paired-overlay-baseline-raw-20261126.txt")
cand_path=Path(r"C:\Projects\repopact-core-wi083-ac7-candidate-overlay-clean-raw-20261126.txt")
base_lines=base_path.read_text(encoding="utf-8").splitlines()
cand_lines=cand_path.read_text(encoding="utf-8").splitlines()
def file_manifest(lines):
    return [line[len("COPY_FILE,"):] for line in lines if line.startswith("COPY_FILE,")]
base_manifest=file_manifest(base_lines)
cand_manifest=file_manifest(cand_lines)
assert len(base_manifest)==1220, len(base_manifest)
assert len(cand_manifest)==1220, len(cand_manifest)
def manifest_bytes(manifest):
    return sum(int(row.rsplit(",",1)[1]) for row in manifest)
assert manifest_bytes(base_manifest)==19624629
assert manifest_bytes(cand_manifest)==19624629
assert base_manifest==cand_manifest, "A/B copied file manifests differ"
Path(r"C:\Projects\repopact-core-wi083-ac7-candidate-artifacts-20261126\candidate-overlay-file-length-manifest.csv").write_text("path_json,length\n"+"\n".join(cand_manifest)+"\n",encoding="utf-8")
def samples(lines, metric_index):
    out={}
    for line in lines:
        if not line.startswith("RAW,"):
            continue
        fields=line.split(",")
        if fields[2]=="sample":
            out.setdefault(fields[1],[]).append(int(fields[metric_index]))
    return out
def p95(v):
    s=sorted(v)
    return s[math.ceil(.95*len(s))-1]
def median(v):
    return statistics.median(v)
base=samples(base_lines,4); cand=samples(cand_lines,4)
assert all(len(base[k])==10 and len(cand[k])==10 for k in ("active_create","active_edit","active_to_blocked"))
summary=[]
summary.append(f"fixture_copy_manifest_files={len(cand_manifest)}")
summary.append(f"fixture_copy_manifest_bytes={manifest_bytes(cand_manifest)}")
summary.append("fixture_copy_manifest_identical_in_A_and_B=true")
summary.append("case,baseline_candidate_validation_median_us,candidate_validation_median_us,reduction_percent,baseline_p95_us,candidate_p95_us")
for case in ("active_create","active_edit","active_to_blocked"):
    bm,cm=median(base[case]),median(cand[case])
    red=100*(bm-cm)/bm
    summary.append(f"{case},{bm},{cm},{red:.3f},{p95(base[case])},{p95(cand[case])}")
all_b=[x for case in base for x in base[case]]
all_c=[x for case in cand for x in cand[case]]
summary.append(f"all_30_samples,{median(all_b)},{median(all_c)},{100*(median(all_b)-median(all_c))/median(all_b):.3f},{p95(all_b)},{p95(all_c)}")
for lines,label in ((base_lines,"baseline"),(cand_lines,"candidate")):
    raw=[line.split(",") for line in lines if line.startswith("RAW,") and line.split(",")[2]=="sample"]
    summary.append(f"{label}_measured_samples={len(raw)}")
    summary.append(f"{label}_all_applicable={all(r[16]=='true' for r in raw)}")
    summary.append(f"{label}_diagnostic_total={sum(int(r[17]) for r in raw)}")
    summary.append(f"{label}_blocking_total={sum(int(r[18]) for r in raw)}")
Path(r"C:\Projects\repopact-core-wi083-ac7-candidate-artifacts-20261126\paired-overlay-comparison-summary.txt").write_text("\n".join(summary)+"\n",encoding="utf-8")
print("\n".join(summary))