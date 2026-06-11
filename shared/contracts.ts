// Wire contract — mirror of src-tauri/src/ipc.rs serde structs.
// Keep field names and types in sync with the Rust side. Bump when the Rust side changes.
export interface AppInfo {
  name: string;
  version: string;
}

export interface DbHealth {
  schema_version: number;
  tables: number;
}
