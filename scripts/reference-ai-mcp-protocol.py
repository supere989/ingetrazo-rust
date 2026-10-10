#!/usr/bin/env python3
import json,runpy
m=runpy.run_path("/work/ingetrazo/scripts/ingetrazo_mcp.py")
m["handle"].__globals__["_call"]=lambda name,args:{"content":[{"type":"text","text":name}],"isError":False,"arguments":args}
requests=[{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}},{"id":"tools","method":"tools/list"},{"id":0,"method":"ping"},{"method":"notifications/initialized"},{"id":4,"method":"unknown"},{"id":5}]
requests += [{"id":6,"method":"tools/call","params":{"name":"run_python","arguments":{"code":"print(1)"}}},{"id":7,"method":"tools/call","params":{"name":"query_model","arguments":None}},{"id":8,"method":"tools/call","params":{"name":"unknown","arguments":False}}]
print(json.dumps([{"request":q,"response":m["handle"](q)}for q in requests],ensure_ascii=False,indent=2))
