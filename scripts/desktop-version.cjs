const STABLE_DESKTOP_VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const DEV_DESKTOP_VERSION_PATTERN = /^\d+\.\d+\.\d+-dev\.\d+$/;

function normalizeDesktopVersion(value) {
  return String(value || "").trim().replace(/^v/, "");
}

function isStableDesktopVersion(value) {
  return STABLE_DESKTOP_VERSION_PATTERN.test(normalizeDesktopVersion(value));
}

function isDevDesktopVersion(value) {
  return DEV_DESKTOP_VERSION_PATTERN.test(normalizeDesktopVersion(value));
}

function parseDevDesktopVersion(value) {
  const version = normalizeDesktopVersion(value);
  if (!isDevDesktopVersion(version)) {
    throw new Error(`Dev desktop version must look like 0.5.8-dev.1, got ${value || "(empty)"}.`);
  }
  return version;
}

module.exports = {
  STABLE_DESKTOP_VERSION_PATTERN,
  DEV_DESKTOP_VERSION_PATTERN,
  normalizeDesktopVersion,
  isStableDesktopVersion,
  isDevDesktopVersion,
  parseDevDesktopVersion,
};
