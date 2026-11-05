"""Quire Python worker.

Speaks newline-delimited JSON on stdin/stdout. The first line written is a
handshake with interpreter info. Each request is:
  {"kind": "dataset", "code": str, "params": {"start": "YYYY-MM-DD", ...}, "cwd": str|None}
and is answered with {"ok", "value": {"columns", "rows"}, "stdout", "error", "ms"}.

A dataset script receives `params` (and `sources`: the other data sources it
names, e.g. sources["Позиции"] — pandas DataFrames, or lists of dicts without
pandas) and leaves its table in `df` or as its last expression: a pandas
DataFrame, a list of dicts, or a dict of lists.

{"kind": "run", "code", "data": {columns, rows}, ...} runs a script on a table
(the SQL workspace's last result, as `df`) and answers with what it printed and
its result (a table, or text).
"""
import ast
import contextlib
import datetime
import decimal
import importlib.util
import io
import json
import math
import os
import sys
import time
import traceback

_proto = sys.stdout
HAS_PANDAS = importlib.util.find_spec("pandas") is not None


def scalar(v):
    if v is None:
        return None
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, str)):
        return v
    if isinstance(v, float):
        return v if math.isfinite(v) else None
    if isinstance(v, decimal.Decimal):
        return float(v)
    if isinstance(v, (datetime.datetime, datetime.date)):
        return v.isoformat()[:10]
    if hasattr(v, "isoformat") and callable(v.isoformat):  # pandas Timestamp
        try:
            return v.isoformat()[:10]
        except Exception:
            return None
    if hasattr(v, "item"):  # numpy scalar
        try:
            return scalar(v.item())
        except Exception:
            pass
    try:
        if v != v:  # NaN / NaT
            return None
    except Exception:
        pass
    return str(v)


def to_table(obj):
    if HAS_PANDAS:
        import pandas as pd
        if isinstance(obj, pd.Series):
            obj = obj.reset_index()
        if isinstance(obj, pd.DataFrame):
            df = obj
            if not isinstance(df.index, pd.RangeIndex):
                df = df.reset_index()
            cols = [str(c) for c in df.columns]
            rows = [[scalar(v) for v in r] for r in df.itertuples(index=False, name=None)]
            return {"columns": cols, "rows": rows}
    if isinstance(obj, dict):
        cols = [str(k) for k in obj.keys()]
        vals = list(obj.values())
        n = max((len(v) for v in vals), default=0)
        return {"columns": cols, "rows": [[scalar(v[i]) if i < len(v) else None for v in vals] for i in range(n)]}
    if isinstance(obj, list) and obj and isinstance(obj[0], dict):
        cols = []
        for r in obj:
            for k in r:
                if k not in cols:
                    cols.append(k)
        return {"columns": [str(c) for c in cols], "rows": [[scalar(r.get(c)) for c in cols] for r in obj]}
    if isinstance(obj, list) and obj and isinstance(obj[0], (list, tuple)):
        head, *rest = obj
        return {"columns": [str(c) for c in head], "rows": [[scalar(v) for v in r] for r in rest]}
    raise TypeError(f"expected a DataFrame, list of dicts or dict of lists, got {type(obj).__name__}")


def frame(t):
    """A {columns, rows} table as a DataFrame (or a list of dicts without pandas)."""
    cols, rows = t.get("columns") or [], t.get("rows") or []
    if HAS_PANDAS:
        import pandas as pd
        return pd.DataFrame(rows, columns=cols)
    return [dict(zip(cols, r)) for r in rows]


def run(code, params, cwd, sources=None):
    ns = {"params": dict(params), "__name__": "__quire__", "sources": {k: frame(v) for k, v in (sources or {}).items()}}
    if HAS_PANDAS:
        import pandas as pd
        ns["pd"] = pd
    if cwd and os.path.isdir(cwd):
        os.chdir(cwd)
    tree = ast.parse(code, "<dataset>", "exec")
    last = None
    if tree.body and isinstance(tree.body[-1], ast.Expr):
        last = ast.Expression(tree.body.pop().value)
    exec(compile(tree, "<dataset>", "exec"), ns)
    result = eval(compile(last, "<dataset>", "eval"), ns) if last is not None else ns.get("df")
    if result is None:
        result = ns.get("df")
    if result is None:
        raise ValueError("The script must leave its table in `df` or as the last expression.")
    return to_table(result)


def run_on(code, data, params, cwd):
    """The SQL workspace's Python: `df` is its last result. Answers a table (a DataFrame,
    a list of dicts…) or the result as text, besides what the script printed."""
    ns = {"params": dict(params or {}), "__name__": "__quire__", "df": frame(data or {})}
    if HAS_PANDAS:
        import pandas as pd
        ns["pd"] = pd
    if cwd and os.path.isdir(cwd):
        os.chdir(cwd)
    tree = ast.parse(code, "<dataset>", "exec")
    last = None
    if tree.body and isinstance(tree.body[-1], ast.Expr):
        last = ast.Expression(tree.body.pop().value)
    exec(compile(tree, "<dataset>", "exec"), ns)
    result = eval(compile(last, "<dataset>", "eval"), ns) if last is not None else None
    if result is None:
        return {"text": None}
    try:
        if HAS_PANDAS:
            import pandas as pd
            if isinstance(result, pd.Series):
                result = result.to_frame()
            if isinstance(result, pd.DataFrame):
                result = result.reset_index() if not isinstance(result.index, pd.RangeIndex) else result
        return {"table": to_table(result)}
    except Exception:
        return {"text": repr(result)[:20000]}


def main():
    _proto.write(json.dumps({"python": sys.version.split()[0], "pandas": HAS_PANDAS}) + "\n")
    _proto.flush()
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        t0 = time.perf_counter()
        out = io.StringIO()
        resp = {"ok": False, "value": None, "stdout": "", "error": None}
        try:
            req = json.loads(line)
            with contextlib.redirect_stdout(out):
                if req.get("kind") == "run":
                    resp["value"] = run_on(req.get("code", ""), req.get("data"), req.get("params"), req.get("cwd"))
                else:
                    resp["value"] = run(req.get("code", ""), req.get("params") or {}, req.get("cwd"), req.get("sources"))
            resp["ok"] = True
        except Exception:
            etype, evalue, tb = sys.exc_info()
            frames = [f for f in traceback.extract_tb(tb) if f.filename == "<dataset>"]
            where = f" (line {frames[-1].lineno})" if frames else ""
            resp["error"] = f"{etype.__name__}{where}: {evalue}"
        resp["stdout"] = out.getvalue()[-4000:]
        resp["ms"] = (time.perf_counter() - t0) * 1000
        _proto.write(json.dumps(resp, default=str) + "\n")
        _proto.flush()


if __name__ == "__main__":
    main()
