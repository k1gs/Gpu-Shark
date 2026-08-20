use std::env;

fn emit_feedback_configuration() {
    println!("cargo:rerun-if-env-changed=GPU_SHARK_FEEDBACK_HOST");
    println!("cargo:rerun-if-env-changed=GPU_SHARK_FEEDBACK_PATH");

    let host = env::var("GPU_SHARK_FEEDBACK_HOST")
        .expect("GPU_SHARK_FEEDBACK_HOST is required to build the feedback client");
    let path = env::var("GPU_SHARK_FEEDBACK_PATH")
        .expect("GPU_SHARK_FEEDBACK_PATH is required to build the feedback client");
    assert!(!host.contains(['\r', '\n']), "invalid feedback host");
    assert!(
        path.starts_with('/') && !path.contains(['\r', '\n']),
        "invalid feedback path"
    );
    println!("cargo:rustc-env=GPU_SHARK_FEEDBACK_HOST={host}");
    println!("cargo:rustc-env=GPU_SHARK_FEEDBACK_PATH={path}");
}

fn main() {
    emit_feedback_configuration();
    if cfg!(target_os = "windows") {
        let mut resource = winres::WindowsResource::new();
        let version = env!("CARGO_PKG_VERSION");
        resource.set("FileVersion", &format!("{version}.0"));
        resource.set("ProductVersion", version);
        resource.set("ProductName", "GPU Shark");
        resource.set("FileDescription", "GPU and CPU telemetry monitor");
        resource.set("LegalCopyright", "Copyright (c) 2026 k1gs");
        resource.set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
        );
        resource.compile().expect("compile Windows resources");
    }
}
