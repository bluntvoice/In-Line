use super::{
    contract::McpError,
    query::{self, QueryState},
    query_types::QueryArgs,
    security::Authorization,
    write_types::BusinessDataArgs,
};
use crate::database::Database;
use serde_json::{json, Value};

pub fn execute(
    state: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    args: BusinessDataArgs,
) -> Result<Value, McpError> {
    if args.action == "export" {
        query::validate_export(auth, &args)?;
    }
    let export_args = args.clone();
    db.mcp_artifact(client,auth,args,||{
        let mut cursor=None;let mut rows=vec![];let mut meta;let mut projection;let mut redacted;
        loop{
            let page=query::tasks(state,db,client,auth,QueryArgs{filters:export_args.filters.clone(),projection:export_args.projection.clone(),limit:Some(100),cursor:cursor.clone(),snapshot:None})?;
            rows.extend(page["items"].as_array().ok_or_else(||McpError::new("internal_error"))?.iter().cloned());
            if rows.len()>10000||serde_json::to_vec(&rows).unwrap().len()>32*1024*1024{return Err(McpError::new("resource_limit"));}
            meta=page["meta"].clone();projection=page["projection"].clone();redacted=page["redactedFields"].clone();
            cursor=page["nextCursor"].as_str().map(str::to_owned);if cursor.is_none(){break;}
        }
        meta["complete"]=json!(true);
        if export_args.format.as_deref()==Some("json"){
            serde_json::to_vec_pretty(&json!({"items":rows,"projection":projection,"redactedFields":redacted,"meta":meta})).map_err(|_|McpError::new("internal_error"))
        }else{
            let keys:Vec<String>=projection.as_array().unwrap().iter().map(|v|v.as_str().unwrap().to_owned()).collect();
            let mut output=String::from("\u{feff}");output+=&keys.iter().map(|s|csv(s)).collect::<Vec<_>>().join(",");output.push('\n');
            for row in rows{output+=&keys.iter().map(|k|csv(&if let Some(s)=row[k].as_str(){s.to_owned()}else{row[k].to_string()})).collect::<Vec<_>>().join(",");output.push('\n');}Ok(output.into_bytes())
        }
    })
}
pub(super) fn csv(value: &str) -> String {
    let mut value = value.to_owned();
    if value.trim_start().starts_with(['=', '+', '-', '@']) || value.starts_with(['\t', '\r', '\n'])
    {
        value.insert(0, '\'');
    }
    format!("\"{}\"", value.replace('"', "\"\""))
}
