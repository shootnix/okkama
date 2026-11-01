use std::path::Path;
fn main(){
 let root=Path::new("/tmp/okkama-audit-20261101");
 let m:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(root.join("metadata.json")).unwrap()).unwrap();
 let mut matches=vec![]; let mut checked=0;
 for p in m["packages"].as_array().unwrap(){
  let name=p["name"].as_str().unwrap();let ver=p["version"].as_str().unwrap();let v:semver::Version=ver.parse().unwrap();
  let dir=root.join("advisory-db/crates").join(name);
  let Ok(files)=std::fs::read_dir(dir) else{continue};
  for file in files{
   let path=file.unwrap().path();if path.extension().is_none_or(|e|e!="md"){continue}
   let s=std::fs::read_to_string(&path).unwrap();let Some(t)=s.split("```toml").nth(1).and_then(|s|s.split("```").next()) else{continue};
   let d:toml::Table=t.parse().unwrap();let a=d["advisory"].as_table().unwrap();if a.contains_key("withdrawn"){continue}
   checked+=1;let vers=d["versions"].as_table().unwrap();
   let safe=["patched","unaffected"].iter().any(|k|vers.get(*k).and_then(|v|v.as_array()).is_some_and(|rs|rs.iter().any(|r|semver::VersionReq::parse(r.as_str().unwrap()).unwrap().matches(&v))));
   if !safe{matches.push(serde_json::json!({"package":name,"version":ver,"id":a["id"].as_str(),"informational":a.get("informational").and_then(|v|v.as_str()),"file":path.to_str()}));}
  }
 }
 let out=serde_json::json!({"method":"local RustSec metadata + Rust semver; not cargo-audit","advisory_package_version_pairs":checked,"matches":matches});
 println!("{}",serde_json::to_string_pretty(&out).unwrap());
}
