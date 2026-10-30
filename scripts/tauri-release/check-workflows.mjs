import { readFile } from "node:fs/promises";
import path from "node:path";

export const WORKFLOWS = ["tauri-ci.yml", "tauri-release.yml"];

export function checkWorkflowText(name, text) {
  const errors = [];
  const require = (condition, message) => {
    if (!condition) errors.push(`${name}: ${message}`);
  };

  require(/^name:\s/m.test(text), "缺少 name");
  require(/^permissions:/m.test(text), "缺少顶层 permissions");
  require(/^concurrency:/m.test(text), "缺少 concurrency（避免并发重复发布）");
  require(!/\|\|\s*true/.test(text), "出现 || true，会掩盖失败");
  require(!/continue-on-error:\s*true/.test(text), "出现 continue-on-error: true，会掩盖失败");
  require(!/pull_request_target/.test(text), "不允许 pull_request_target");

  const uses = [...text.matchAll(/uses:\s*(\S+)/g)].map((match) => match[1]);
  for (const ref of uses) {
    // 本地可复用 workflow（./.github/workflows/...）本来就不带版本引用。
    if (ref.startsWith("./")) continue;
    require(ref.includes("@"), `actions 引用缺少版本：${ref}`);
  }

  const uploads = [...text.matchAll(/uses:\s*actions\/upload-artifact@[^\n]+\n(?:.*\n)*?\s+with:\n((?:\s+.*\n)+)/g)];
  for (const upload of uploads) {
    require(/if-no-files-found:\s*error/.test(upload[1]), "upload-artifact 必须设置 if-no-files-found: error");
  }

  if (name === "tauri-release.yml") {
    require(/tags:\s*\n\s+-\s*"tauri-v\*"/.test(text), "release 必须只接受 tauri-v* Tag");
    require(/gh release create/.test(text) && /--draft/.test(text), "release 必须先建 Draft");
    require(/inputs\.publish == true/.test(text), "正式发布必须显式要求 publish=true");
    require(/create_updater_artifacts/.test(text), "release 必须区分是否生成 updater 产物");
  }

  return { ok: errors.length === 0, errors };
}

export async function checkWorkflows(root) {
  const errors = [];
  for (const name of WORKFLOWS) {
    const file = path.join(root, ".github/workflows", name);
    let text;
    try {
      text = await readFile(file, "utf8");
    } catch (error) {
      errors.push(`${name}: 无法读取（${error.message}）`);
      continue;
    }
    errors.push(...checkWorkflowText(name, text).errors);
  }
  return { ok: errors.length === 0, errors };
}
