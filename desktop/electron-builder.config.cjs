const path = require("node:path");
const { parseDevDesktopVersion } = require("../scripts/desktop-version.cjs");

function firstNonEmpty(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

const releaseChannel = firstNonEmpty(process.env.AI_NOVEL_RELEASE_CHANNEL, "beta").toLowerCase();
const isBetaRelease = releaseChannel === "beta";
const isDevRelease = releaseChannel === "dev";
const isPrereleaseChannel = isBetaRelease || isDevRelease;
const desktopVersionOverride = isDevRelease
  ? parseDevDesktopVersion(process.env.AI_NOVEL_DESKTOP_VERSION)
  : "";
const githubOwner = firstNonEmpty(process.env.AI_NOVEL_GITHUB_OWNER, isDevRelease ? "" : "ExplosiveCoderflome");
const githubRepo = firstNonEmpty(process.env.AI_NOVEL_GITHUB_REPO, isDevRelease ? "" : "AI-Novel-Writing-Assistant");
if (isDevRelease && (!githubOwner || !githubRepo)) {
  throw new Error("Dev desktop packages require AI_NOVEL_GITHUB_OWNER and AI_NOVEL_GITHUB_REPO.");
}
const windowsSigningLink = firstNonEmpty(
  process.env.CSC_LINK,
  process.env.WIN_CSC_LINK,
  process.env.AI_NOVEL_WINDOWS_CSC_LINK,
  process.env.AI_NOVEL_WINDOWS_CSC_FILE,
);
const allowUnsignedRelease =
  firstNonEmpty(
    process.env.AI_NOVEL_ALLOW_UNSIGNED_RELEASE,
    process.env.AI_NOVEL_ALLOW_UNSIGNED_WINDOWS_RELEASE,
  ).toLowerCase() === "true";
const macSigningIdentityName = firstNonEmpty(process.env.CSC_NAME, process.env.AI_NOVEL_MAC_CSC_NAME);
const hasSigningMaterial = Boolean(windowsSigningLink || macSigningIdentityName);
const hasMacSigningMaterial = Boolean(firstNonEmpty(process.env.CSC_LINK) || macSigningIdentityName);
const hasMacNotarizeCredentials = Boolean(
  firstNonEmpty(process.env.APPLE_TEAM_ID)
  && (
    (firstNonEmpty(process.env.APPLE_ID) && firstNonEmpty(process.env.APPLE_APP_SPECIFIC_PASSWORD))
    || (firstNonEmpty(process.env.APPLE_API_KEY) && firstNonEmpty(process.env.APPLE_API_KEY_ID) && firstNonEmpty(process.env.APPLE_API_ISSUER))
  ),
);
const builderIconPath = path.join("builder", "app-icon.ico");
const builderMacIconPath = path.join("builder", "app-icon.png");
const builderMacEntitlementsPath = path.join("builder", "entitlements.mac.plist");

if (!isPrereleaseChannel && !hasSigningMaterial && !allowUnsignedRelease) {
  throw new Error(
    "Public desktop releases require signing material. Provide CSC_LINK/WIN_CSC_LINK (or CSC_NAME on macOS), or explicitly opt in to an unsigned release.",
  );
}

module.exports = {
  appId: "com.ai-novel.desktop",
  productName: "Biz Novel Studio",
  directories: {
    app: "build/app",
    output: "build/dist",
    buildResources: "builder",
  },
  files: [
    "dist/**/*",
    "package.json",
    "node_modules/.prisma/**/*",
  ],
  extraResources: [
    {
      from: "builder/app-icon.ico",
      to: "icons/app-icon.ico",
    },
    {
      from: "build/resources/app-update.yml",
      to: "app-update.yml",
    },
    {
      from: "build/resources/client",
      to: "client",
      filter: ["**/*"],
    },
  ],
  asar: true,
  asarUnpack: [
    "node_modules/**/*.node",
  ],
  npmRebuild: true,
  nativeRebuilder: "sequential",
  extraMetadata: {
    main: "dist/main.js",
    ...(desktopVersionOverride ? { version: desktopVersionOverride } : {}),
    aiNovelDesktop: {
      // Squirrel.Mac only installs updates signed by the same Developer ID, so ad-hoc builds must update manually.
      macAutoUpdate: hasMacSigningMaterial,
    },
  },
  publish: [
    {
      provider: "github",
      owner: githubOwner,
      repo: githubRepo,
      releaseType: isPrereleaseChannel ? "prerelease" : "release",
    },
  ],
  electronUpdaterCompatibility: ">=2.16",
  generateUpdatesFilesForAllChannels: false,
  win: {
    icon: builderIconPath,
    // Keep EXE resource editing enabled for unsigned builds so Windows uses the app icon and metadata.
    signAndEditExecutable: true,
    target: [
      {
        target: "nsis",
        arch: ["x64"],
      },
      {
        target: "portable",
        arch: ["x64"],
      },
    ],
  },
  nsis: {
    artifactName: "${productName}-${version}-setup-${arch}.${ext}",
    oneClick: false,
    perMachine: false,
    allowToChangeInstallationDirectory: true,
    allowElevation: true,
    createDesktopShortcut: true,
    createStartMenuShortcut: true,
    deleteAppDataOnUninstall: false,
    runAfterFinish: true,
    installerIcon: builderIconPath,
    uninstallerIcon: builderIconPath,
    installerHeaderIcon: builderIconPath,
  },
  portable: {
    artifactName: "${productName}-${version}-portable-${arch}.${ext}",
  },
  mac: {
    icon: builderMacIconPath,
    category: "public.app-category.productivity",
    // electron-updater picks the Apple Silicon package by the `arm64` marker in the file name.
    artifactName: "${productName}-${version}-mac-${arch}.${ext}",
    // `-` is ad-hoc signing: Apple Silicon refuses to launch fully unsigned binaries.
    ...(hasMacSigningMaterial ? {} : { identity: "-" }),
    hardenedRuntime: hasMacSigningMaterial,
    entitlements: builderMacEntitlementsPath,
    entitlementsInherit: builderMacEntitlementsPath,
    notarize: hasMacSigningMaterial && hasMacNotarizeCredentials,
    target: [
      {
        target: "dmg",
        arch: ["arm64", "x64"],
      },
      {
        target: "zip",
        arch: ["arm64", "x64"],
      },
    ],
  },
  dmg: {
    artifactName: "${productName}-${version}-mac-${arch}.${ext}",
  },
};
