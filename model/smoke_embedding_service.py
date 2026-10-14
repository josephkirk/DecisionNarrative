"""Run a real local HTTP inference smoke test and always stop the service."""

from __future__ import annotations

import json
import math
import os
import subprocess
import sys
import time
import urllib.request


def main() -> None:
    process = subprocess.Popen(
        [sys.executable, "model/embedding_service.py"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    base = "http://127.0.0.1:4318"
    started = time.monotonic()
    try:
        while time.monotonic() - started < 180:
            try:
                with urllib.request.urlopen(base + "/health", timeout=2) as response:
                    health = json.load(response)
                if health.get("status") in ("not_loaded", "ok"):
                    break
            except Exception:
                time.sleep(1)
        else:
            raise RuntimeError("Embedding service did not become ready")

        payload = json.dumps(
            {"input": "A detective follows the receipt.", "input_type": "query"}
        ).encode()
        request = urllib.request.Request(
            base + "/v1/embeddings",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=180) as response:
            result = json.load(response)

        vector = result["embeddings"][0]
        norm = math.sqrt(sum(value * value for value in vector))
        assert len(vector) == 768
        assert all(math.isfinite(value) for value in vector)
        assert abs(norm - 1.0) < 1e-5
        print(
            json.dumps(
                {
                    "health": health,
                    "model": result["model"],
                    "revision": result["model_revision"],
                    "dimensions": len(vector),
                    "finite": True,
                    "norm": norm,
                },
                indent=2,
            )
        )
    finally:
        if os.name == "nt":
            # uv's Windows Python shim starts a child interpreter; stop the
            # owned process tree so the HTTP server cannot outlive this test.
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                check=False,
                capture_output=True,
            )
        else:
            process.terminate()
        process.wait(timeout=10)


if __name__ == "__main__":
    main()
