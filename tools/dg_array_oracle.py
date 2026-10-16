"""CPython valid-input oracle for unchanged CV04; no MiniPy simulation."""
import contextlib
import hashlib
import io
import json
import pathlib
import types

root = pathlib.Path(__file__).resolve().parents[1]
source = (root / "fixtures/cv/04_first_matching_result.py").read_bytes()
cases = [([], "0\n"), ([{"score": 81, "id": 7}], "7\n"),
         ([{"score": 80}, {"score": 82, "id": 9}], "9\n"),
         ([{"score": 1}, {"score": 2}], "0\n"),
         ([{"score": 81, "id": 7}, None, {"score": "bad"}], "7\n")]
results = []
for rows, expected in cases:
    calls = []
    def search(request_id):
        assert request_id == 7
        calls.append("search")
        return {"results": rows}
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        exec(compile(source, "CV04", "exec"), {"tools": types.SimpleNamespace(search=search), "request_id": 7})
    assert out.getvalue() == expected and calls == ["search"]
    results.append({"rows": rows, "stdout": out.getvalue(), "calls": calls})
print(json.dumps({"source_sha256": hashlib.sha256(source).hexdigest(), "cases": results}, indent=2))
