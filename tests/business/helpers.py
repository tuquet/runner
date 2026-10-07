import os
import json
import urllib.request
import urllib.error

IDENTITY_FILE = os.path.expanduser("~/.specter/system/.identity.json")
RUNNER_BASE_URL = "http://127.0.0.1:8765"

def load_cloud_credentials():
    if os.path.exists(IDENTITY_FILE):
        with open(IDENTITY_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    return {
        "cloud_url": "https://dswhacsoaxgpfnkaxnhz.supabase.co",
        "api_key": "dummy_anon_key",
        "tenant_id": "b0000000-0000-0000-0000-000000000001",
        "device_id": "a0a2989b-4b5f-436e-a369-8b7cc184f194",
        "device_token": "dummy_token"
    }

def call_cloud_rpc(func_name, payload):
    creds = load_cloud_credentials()
    url = f"{creds['cloud_url']}/rest/v1/rpc/{func_name}"
    headers = {
        "apikey": creds["api_key"],
        "Authorization": f"Bearer {creds['api_key']}",
        "Content-Type": "application/json"
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        error_body = e.read().decode("utf-8")
        try:
            return {"error_http_code": e.code, "error_body": json.loads(error_body)}
        except Exception:
            return {"error_http_code": e.code, "error_body": error_body}

def call_runner_api(method, endpoint, payload=None):
    url = f"{RUNNER_BASE_URL}{endpoint}"
    headers = {"Content-Type": "application/json"}
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req) as resp:
            raw = resp.read().decode("utf-8")
            if not raw.strip():
                return resp.status, {}
            try:
                return resp.status, json.loads(raw)
            except Exception:
                return resp.status, {"raw": raw}
    except urllib.error.HTTPError as e:
        error_body = e.read().decode("utf-8")
        try:
            return e.code, json.loads(error_body)
        except Exception:
            return e.code, {"raw": error_body}
