/** The native command owns the file dialog; callers cannot supply executable paths. */
export async function selectNativeJavaRuntime(): Promise<number | null> {
  const {invoke} = await import("@tauri-apps/api/core");
  return invoke<number | null>("select_java_runtime");
}
