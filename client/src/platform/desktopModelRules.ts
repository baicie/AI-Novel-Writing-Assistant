/**
 * 桌面端模型配置的共用规则：地址许可与错误文案。
 * 这里只放跨组件复用的纯函数，避免 onboarding 反向依赖 pages/settings。
 */

/**
 * 明文 http 地址需要显式允许，否则内核会拒绝（自建网关常用 http）。
 * 元数据地址仍会被内核拒绝，这个开关不会放开它们。
 */
export function needsPlaintextHttpPermission(endpoint: string): boolean {
  return /^http:\/\//i.test(endpoint.trim());
}

/** 把内核返回的错误对象整理成一句用户能看懂的话（带机器码，便于排查）。 */
export function formatDiscoveryError(error: unknown): string {
  const record = (error ?? {}) as { code?: string; message?: string };
  if (record.message && record.code) return `${record.message}（${record.code}）`;
  if (record.message) return record.message;
  if (error instanceof Error) return error.message;
  return "获取模型列表失败，可以手动填写模型名称。";
}
