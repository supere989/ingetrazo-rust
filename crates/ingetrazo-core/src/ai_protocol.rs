//! Pure original-compatible MCP routing. Runtime tool execution is supplied by
//! the UI-thread bridge; this module does not open sockets or execute scripts.
use serde_json::{json, Value};
fn truth(value:&Value)->bool{match value{Value::Null=>false,Value::Bool(b)=>*b,Value::Number(n)=>n.as_f64()!=Some(0.),Value::String(s)=>!s.is_empty(),Value::Array(a)=>!a.is_empty(),Value::Object(o)=>!o.is_empty()}}
pub const PROTOCOL: &str = "2024-11-05";
pub fn metadata() -> Value { serde_json::from_str(include_str!("../../../resources/ai_mcp_reference.json")).expect("compiled MCP reference") }
pub fn handle(message: &Value, mut call: impl FnMut(&str, &Value) -> Value) -> Option<Value> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let result = match message.get("method").and_then(Value::as_str) {
        Some("initialize") => {let reference=metadata(); json!({"protocolVersion":PROTOCOL,"capabilities":{"tools":{}},"serverInfo":{"name":"ingetrazo","version":"1.0.0"},"instructions":reference["instructions"]})},
        Some("tools/list") => json!({"tools":metadata()["tools"]}),
        Some("tools/call") => {let params=&message["params"]; let name=params["name"].as_str().unwrap_or("");call(name,params.get("arguments").filter(|v| truth(v)).unwrap_or(&json!({})))},
        Some("ping") => json!({}),
        method => {if id.is_null(){return None;}let method=method.unwrap_or("None");return Some(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("unknown {method}")}}));},
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
pub fn text_result(text:impl Into<String>,error:bool)->Value{json!({"content":[{"type":"text","text":text.into()}],"isError":error})}
/// Format a live bridge reply. Screenshot image encoding is a native runtime
/// responsibility and must not be substituted with an inspection summary.
pub fn bridge_tool_result(name:&str,reply:&Value)->Result<Value,String>{
 if reply["ok"]!=true{return Ok(text_result(reply["error"].as_str().unwrap_or("None"),true));}
 let result=reply.get("result").filter(|v| truth(v)).cloned().unwrap_or_else(||json!({}));
 if name=="screenshot"&&result["path"].as_str().is_some(){return Err("Screenshot requires native captured-image transport".into());}
 if name=="run_python"{let mut parts=Vec::new();if let Some(stdout)=result["stdout"].as_str().filter(|s|!s.is_empty()){parts.push(stdout.trim_end().to_owned());}if let Some(stderr)=result["stderr"].as_str().filter(|s|!s.is_empty()){parts.push(format!("stderr:\n{}",stderr.trim_end()));}let error=result["error"].as_str().filter(|s|!s.is_empty());if error.is_some(){return Err("Execution error has no verified atomic rollback acknowledgement".into());}let changed=match result.get("changed"){Some(Value::Bool(true))=>"True",Some(Value::Bool(false))=>"False",_=>"None"};parts.push(format!("(changed: {changed})"));return Ok(text_result(parts.join("\n"),error.is_some()));}
 Ok(text_result(serde_json::to_string_pretty(&result).map_err(|e|e.to_string())?,false))
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn protocol_routes_without_executing_notifications(){let mut calls=0;let answer=handle(&json!({"method":"notifications/initialized"}),|_,_|{calls+=1;json!({})});assert!(answer.is_none());assert_eq!(calls,0);let answer=handle(&json!({"id":"a","method":"tools/call","params":{"name":"run_python","arguments":{"code":"raise RuntimeError()"}}}),|name,args|{assert_eq!(name,"run_python");assert_eq!(args["code"],"raise RuntimeError()");text_result("Python interpreter not implemented",true)}).unwrap();assert_eq!(answer["id"],"a");assert_eq!(answer["result"]["isError"],true);}
 #[test]fn execution_error_cannot_claim_rollback(){let partial=json!({"ok":true,"result":{"error":"existing material changed before failure","changed":false}});assert!(bridge_tool_result("run_python",&partial).is_err());let refused=json!({"ok":false,"error":"Native interpreter and atomic journal are not implemented"});let answer=bridge_tool_result("run_python",&refused).unwrap();assert_eq!(answer["isError"],true);assert!(!answer["content"][0]["text"].as_str().unwrap().contains("ROLLED BACK"));}
 #[test]fn screenshot_does_not_fabricate_image(){assert!(bridge_tool_result("screenshot",&json!({"ok":true,"result":{"path":"viewport.png"}})).is_err());}
}
