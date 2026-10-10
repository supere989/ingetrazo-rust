//! Frozen core.ai provider wire compatibility. No network or document execution.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const PROVIDERS: [&str; 7] = ["groq", "anthropic", "openai", "gemini", "openrouter", "deepseek", "ollama"];
pub const USER_AGENT: &str = "IngeTrazo/0.5.7 (+https://ingetrazo.com)";
pub const RETRY_STATUSES: [u16; 6] = [429, 500, 502, 503, 504, 529];
pub const RETRY_BACKOFF: [u64; 4] = [2, 4, 8, 16];

pub fn detect_provider(key: &str) -> &'static str {
    if key.is_empty() { "ollama" }
    else if key.starts_with("gsk_") { "groq" }
    else if key.starts_with("sk-ant-") { "anthropic" }
    else if key.starts_with("AIza") { "gemini" }
    else if key.starts_with("sk-or-") { "openrouter" }
    else if key.starts_with("sk-") { "openai" }
    else { "anthropic" }
}
pub fn default_model(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "groq" => "openai/gpt-oss-120b", "anthropic" => "claude-sonnet-5", "openai" => "gpt-4o",
        "gemini" => "gemini-2.5-flash", "openrouter" => "anthropic/claude-sonnet-5",
        "deepseek" => "deepseek-chat", "ollama" => "llama3.2", _ => return None,
    })
}
pub fn supports_vision(provider: &str, model: &str) -> bool {
    let model = model.to_lowercase();
    matches!(provider, "anthropic" | "openai" | "gemini" | "openrouter") ||
    ["llama-4", "scout", "maverick", "llava", "vision", "-vl", "vl-", "gemma3", "gemma-3", "pixtral"].iter().any(|t| model.contains(t))
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: String, pub text: String,
    #[serde(default,skip_serializing_if="Option::is_none")] pub image_b64: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub image_mime: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub image_png_b64: Option<String>,
}
impl Message {
    pub fn text(role: &str, text: impl Into<String>) -> Self { Self {role: role.into(), text: text.into(), image_b64: None, image_mime: None, image_png_b64: None} }
    fn image(&self) -> Option<(&str, &str)> {
        if let Some(img) = self.image_b64.as_deref().filter(|s| !s.is_empty()) { return Some((img,self.image_mime.as_deref().unwrap_or("image/png"))); }
        self.image_png_b64.as_deref().filter(|s| !s.is_empty()).map(|img| (img,if img.starts_with("/9j/") {"image/jpeg"} else {"image/png"}))
    }
}
pub fn slim_messages(messages: &[Message], vision: bool) -> Vec<Message> {
    let last = messages.iter().rposition(|m| m.image_png_b64.as_ref().is_some_and(|s| !s.is_empty()));
    messages.iter().enumerate().map(|(i,m)| { let mut m=m.clone(); if !vision {m.image_b64=None;m.image_mime=None;m.image_png_b64=None;} else if Some(i)!=last && m.image_png_b64.as_ref().is_some_and(|s| !s.is_empty()) {m.image_png_b64=None;} m }).collect()
}
// Deliberately no Debug: headers may contain credentials.
pub struct Request {pub url: String,pub headers: BTreeMap<String,String>,pub payload: Value}
fn base(provider: &str, local: &str) -> Result<String,String> {
    Ok(match provider {"groq"=>"https://api.groq.com/openai/v1","openai"=>"https://api.openai.com/v1","gemini"=>"https://generativelanguage.googleapis.com/v1beta/openai","openrouter"=>"https://openrouter.ai/api/v1","deepseek"=>"https://api.deepseek.com/v1","ollama"=>return Ok(format!("{}/v1",local.trim_end_matches('/'))),_=>return Err(format!("Unknown provider {provider}"))}.into())
}
pub fn build_request(provider:&str,model:&str,key:&str,system:&str,messages:&[Message],local:&str,max_tokens:u64)->Result<Request,String>{
    let mut headers=BTreeMap::from([("Content-Type".into(),"application/json".into()),("User-Agent".into(),USER_AGENT.into())]);
    if provider=="anthropic" {
        headers.insert("x-api-key".into(),key.into());headers.insert("anthropic-version".into(),"2023-06-01".into());
        let content:Vec<_>=messages.iter().map(|m|{let mut blocks=Vec::new();if let Some((data,mime))=m.image(){blocks.push(json!({"type":"image","source":{"type":"base64","media_type":mime,"data":data}}));}blocks.push(json!({"type":"text","text":m.text}));json!({"role":m.role,"content":blocks})}).collect();
        return Ok(Request{url:"https://api.anthropic.com/v1/messages".into(),headers,payload:json!({"model":model,"max_tokens":max_tokens,"system":system,"messages":content})});
    }
    let mut content=vec![json!({"role":"system","content":system})];
    for m in messages {let c=if let Some((img,mime))=m.image(){json!([{"type":"image_url","image_url":{"url":format!("data:{mime};base64,{img}")}},{"type":"text","text":m.text}])}else{json!(m.text)};content.push(json!({"role":m.role,"content":c}));}
    if !key.is_empty(){headers.insert("Authorization".into(),format!("Bearer {key}"));}
    Ok(Request{url:format!("{}/chat/completions",base(provider,local)?),headers,payload:json!({"model":model,"max_tokens":max_tokens,"messages":content})})
}
pub fn parse_reply(provider:&str,data:&Value)->Result<String,String>{
    if provider=="anthropic" {return Ok(data["content"].as_array().map(|a|a.iter().filter_map(|b|b["text"].as_str()).collect::<String>()).unwrap_or_default());}
    let choice=data["choices"].as_array().and_then(|a|a.first()).ok_or_else(||format!("Missing provider choices: {}",data.to_string().chars().take(400).collect::<String>()))?;
    Ok(choice["message"]["content"].as_str().unwrap_or_default().into())
}
pub fn extract_code(text:&str)->Option<&str>{let(marker,start)=if let Some(s)=text.find("```python"){("```python",s)}else{("```py",text.find("```py")?)};let rest=&text[start+marker.len()..];let code=rest[..rest.find("```")?].trim_matches('\n');(!code.is_empty()).then_some(code)}
pub fn truncated_code(text:&str)->bool{["```python","```py"].iter().any(|marker|text.find(marker).is_some_and(|start|!text[start+marker.len()..].contains("```")))}

pub fn compact_messages(messages:&[Message],keep_last:usize)->Vec<Message>{
    let coded=messages.iter().enumerate().filter_map(|(i,m)|(m.role=="assistant"&&extract_code(&m.text).is_some()).then_some(i)).collect::<Vec<_>>();
    let old=&coded[..coded.len().saturating_sub(keep_last)];
    messages.iter().enumerate().map(|(i,m)|{let mut m=m.clone();if old.contains(&i){let marker=if m.text.contains("```python"){"```python"}else{"```py"};let start=m.text.find(marker).unwrap();let end=start+marker.len()+m.text[start+marker.len()..].find("```").unwrap();m.text=format!("{}[receta ya ejecutada — código omitido]{}",&m.text[..start],&m.text[end+3..]).trim().into();}m}).collect()
}
pub fn strip_thoughts(text:&str)->String{let mut text=text.to_owned();for tag in ["thought","thinking","think"]{let open=format!("<{tag}>");let close=format!("</{tag}>");while let Some(start)=text.find(&open){let tail=text[start..].find(&close).map(|e|start+e+close.len()).unwrap_or(text.len());text.replace_range(start..tail,"");}if let Some(end)=text.find(&close){text=text[end+close.len()..].into();}}text.trim().into()}

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {pub provider:String,pub api_key:String,pub model:String,pub local_url:String,pub screenshots:bool,pub keys:BTreeMap<String,String>,pub models:BTreeMap<String,String>}
impl Default for Settings {fn default()->Self{Self{provider:"auto".into(),api_key:String::new(),model:String::new(),local_url:"http://localhost:11434".into(),screenshots:true,keys:BTreeMap::new(),models:BTreeMap::new()}}}
impl Settings {
    pub fn effective_provider(&self)->&str{if self.provider=="auto"{detect_provider(self.api_key.trim())}else{&self.provider}}
    pub fn effective_model(&self)->&str{if self.model.trim().is_empty(){default_model(self.effective_provider()).unwrap_or("")}else{self.model.trim()}}
    pub fn switch_provider(&mut self,provider:&str){let old=self.effective_provider().to_owned();if old!="ollama"{if !self.api_key.trim().is_empty(){self.keys.insert(old.clone(),self.api_key.clone());}if !self.model.trim().is_empty(){self.models.insert(old,self.model.trim().into());}}self.provider=provider.into();if provider!="auto"{self.api_key=self.keys.get(provider).cloned().unwrap_or_default();self.model=self.models.get(provider).cloned().unwrap_or_default();}}
}
#[derive(Default)]pub struct Session {pub settings:Settings,pub messages:Vec<Message>,pub prompt:String,pub status:String}
impl Session {
    /// Stage a user turn locally. No HTTP call and no document mutation.
    pub fn stage_prompt(&mut self)->Result<(),String>{if self.prompt.trim().is_empty(){return Err("Empty prompt".into());}self.messages.push(Message::text("user",std::mem::take(&mut self.prompt)));self.status="Request staged locally; provider transport and recipe execution pending".into();Ok(())}
    pub fn request(&self,system:&str,max_tokens:u64)->Result<Request,String>{let p=self.settings.effective_provider();let m=self.settings.effective_model();build_request(p,m,self.settings.api_key.trim(),system,&compact_messages(&slim_messages(&self.messages,supports_vision(p,m)),2),&self.settings.local_url,max_tokens)}
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn request_diet_preserves_stored_images_and_provider_keys(){let mut s=Session::default();s.settings.api_key="gsk_synthetic".into();s.settings.model="custom".into();s.settings.switch_provider("gemini");s.settings.api_key="AIzaSYNTHETIC".into();s.settings.switch_provider("groq");assert_eq!(s.settings.api_key,"gsk_synthetic");assert_eq!(s.settings.model,"custom");let mut m=Message::text("user","photo");m.image_b64=Some("QUJD".into());s.messages.push(m);let before=s.messages.clone();let r=s.request("SYS",4096).unwrap();assert!(r.payload["messages"][1]["content"].is_string());assert_eq!(s.messages,before);}
 #[test]fn truncated_reply_is_not_executable_and_staging_does_not_report_execution(){assert!(truncated_code("```python\na=1"));assert_eq!(extract_code("```python\na=1"),None);let mut s=Session::default();assert!(s.stage_prompt().is_err());s.prompt="make a table".into();s.stage_prompt().unwrap();assert_eq!(s.messages.len(),1);assert!(s.status.contains("pending"));}
 #[test]fn provider_replies_are_not_assumed_successful(){assert!(parse_reply("groq",&json!({"choices":[]})).is_err());assert_eq!(parse_reply("anthropic",&json!({"content":[{"type":"text","text":"a"},{"type":"tool_use"},{"type":"text","text":"b"}]})).unwrap(),"ab");}
}
