#!/usr/bin/env python3
"""Offline pinned original core.ai wire oracle; synthetic credentials only."""
import json,os,sys
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from core import ai
messages=[{"role":"user","text":"reference","image_b64":"QUJD","image_mime":"image/jpeg"},{"role":"assistant","text":"```python\nprint(1)\n```"},{"role":"user","text":"old shot","image_png_b64":"QUJD"},{"role":"user","text":"latest","image_png_b64":"/9j/QUJD"}]
cases=[]
for provider in ai.PROVIDERS:
 for vision in (True,False):
  slim=ai.slim_messages(messages,vision)
  url,headers,payload=ai.build_request(provider,ai.DEFAULT_MODELS[provider],"synthetic-test-key","SYS",slim,ollama_url="http://localhost:1234///",max_tokens=8192)
  cases.append({"provider":provider,"vision":vision,"messages":messages,"slim":slim,"model":ai.DEFAULT_MODELS[provider],"url":url,"headers":headers,"payload":json.loads(payload)})
keys=["","gsk_test","sk-ant-test","AIzaTEST","sk-or-test","sk-test","other"," GSK_test"]
models=[("groq","llama-4-scout"),("groq","openai/gpt-oss-120b"),("ollama","qwen-vl"),("ollama","llama3.2"),("deepseek","GEMMA-3"),("anthropic","")]
texts=["no code","```py\na=1\n```","```python\nprint(1)","```py\nfirst\n``` then ```python\nsecond\n```","```python\n\n```"]
compacted=[{"role":"assistant","text":"intent ```python\nprint(1)\n``` done"},{"role":"user","text":"feedback"},{"role":"assistant","text":"```py\nprint(2)\n```"},{"role":"assistant","text":"```python\nprint(3)\n```"}]
thoughts=["before <think>hidden</think> after","start <thinking>unclosed","stray</thought>answer","<thought>a</thought><think>b</think>c","no tags"]
print(json.dumps({"compact":[{"messages":compacted,"keep":k,"output":ai.compact_messages(compacted,k)}for k in [0,1,2]],"thoughts":[[t,ai.strip_thoughts(t)]for t in thoughts],"requests":cases,"detect":[[k,ai.detect_provider(k)]for k in keys],"vision":[[p,m,ai.supports_vision(p,m)]for p,m in models],"code":[{"text":t,"extract":ai.extract_code(t),"truncated":ai.truncated_code(t)}for t in texts]},indent=2))
