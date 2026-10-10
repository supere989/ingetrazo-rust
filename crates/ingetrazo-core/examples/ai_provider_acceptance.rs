use ingetrazo_core::ai::*;
use serde_json::{json,Value};
fn main(){let source:Value=serde_json::from_slice(&std::fs::read(std::env::args().nth(1).expect("oracle JSON")).unwrap()).unwrap();
 let mut requests=Vec::new();for c in source["requests"].as_array().unwrap(){let provider=c["provider"].as_str().unwrap();let messages:Vec<Message>=serde_json::from_value(c["messages"].clone()).unwrap();let slim=slim_messages(&messages,c["vision"].as_bool().unwrap());let request=build_request(provider,default_model(provider).unwrap(),"synthetic-test-key","SYS",&slim,"http://localhost:1234///",8192).unwrap();requests.push(json!({"provider":provider,"vision":c["vision"],"messages":messages,"slim":slim,"model":default_model(provider).unwrap(),"url":request.url,"headers":request.headers,"payload":request.payload}));}
 let detect=source["detect"].as_array().unwrap().iter().map(|c|{let k=c[0].as_str().unwrap();json!([k,detect_provider(k)])}).collect::<Vec<_>>();
 let vision=source["vision"].as_array().unwrap().iter().map(|c|{let p=c[0].as_str().unwrap();let m=c[1].as_str().unwrap();json!([p,m,supports_vision(p,m)])}).collect::<Vec<_>>();
 let code=source["code"].as_array().unwrap().iter().map(|c|{let t=c["text"].as_str().unwrap();json!({"text":t,"extract":extract_code(t),"truncated":truncated_code(t)})}).collect::<Vec<_>>();
 let compact=source["compact"].as_array().unwrap().iter().map(|c|{let messages:Vec<Message>=serde_json::from_value(c["messages"].clone()).unwrap();let keep=c["keep"].as_u64().unwrap()as usize;json!({"messages":messages,"keep":keep,"output":compact_messages(&messages,keep)})}).collect::<Vec<_>>();
 let thoughts=source["thoughts"].as_array().unwrap().iter().map(|c|{let t=c[0].as_str().unwrap();json!([t,strip_thoughts(t)])}).collect::<Vec<_>>();
 println!("{}",serde_json::to_string_pretty(&json!({"compact":compact,"thoughts":thoughts,"requests":requests,"detect":detect,"vision":vision,"code":code})).unwrap());}
