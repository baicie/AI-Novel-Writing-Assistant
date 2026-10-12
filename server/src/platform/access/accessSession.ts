import { createHmac, timingSafeEqual, createHash } from "node:crypto";
import type { NextFunction, Request, Response } from "express";

export const ACCESS_COOKIE_NAME = "ai_novel_access";
const SESSION_TTL_MS = 30 * 24 * 60 * 60 * 1000;
const ATTEMPT_WINDOW_MS = 15 * 60 * 1000;
const MAX_FAILED_ATTEMPTS = 20;

const failedAttempts = new Map<string, { count: number; resetAt: number }>();

export function readAccessPassword(): string | null {
  const value = process.env.ACCESS_PASSWORD?.trim();
  return value ? value : null;
}

export function isAccessGateEnabled(): boolean {
  return readAccessPassword() !== null;
}

export function resetAccessAttemptLimiter(): void {
  failedAttempts.clear();
}

function digest(value: string): Buffer {
  return createHash("sha256").update(value).digest();
}

export function accessPasswordMatches(input: string): boolean {
  const expected = readAccessPassword();
  if (!expected) {
    return false;
  }
  return timingSafeEqual(digest(input), digest(expected));
}

function signPayload(payload: string, password: string): string {
  return createHmac("sha256", password).update(payload).digest("base64url");
}

export function createAccessToken(now = Date.now()): string {
  const password = readAccessPassword();
  if (!password) {
    throw new Error("ACCESS_PASSWORD is not configured.");
  }
  const payload = Buffer.from(JSON.stringify({ exp: now + SESSION_TTL_MS })).toString("base64url");
  return `${payload}.${signPayload(payload, password)}`;
}

export function verifyAccessToken(token: string | undefined, now = Date.now()): boolean {
  const password = readAccessPassword();
  if (!password || !token) {
    return false;
  }
  const separator = token.lastIndexOf(".");
  if (separator <= 0) {
    return false;
  }
  const payload = token.slice(0, separator);
  const signature = token.slice(separator + 1);
  const expected = signPayload(payload, password);
  const actualBuffer = Buffer.from(signature);
  const expectedBuffer = Buffer.from(expected);
  if (actualBuffer.length !== expectedBuffer.length || !timingSafeEqual(actualBuffer, expectedBuffer)) {
    return false;
  }
  try {
    const parsed = JSON.parse(Buffer.from(payload, "base64url").toString("utf8")) as { exp?: unknown };
    return typeof parsed.exp === "number" && parsed.exp > now;
  } catch {
    return false;
  }
}

function readCookie(header: string | undefined, name: string): string | undefined {
  if (!header) {
    return undefined;
  }
  for (const part of header.split(";")) {
    const separator = part.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    if (part.slice(0, separator).trim() === name) {
      return decodeURIComponent(part.slice(separator + 1).trim());
    }
  }
  return undefined;
}

export function requestHasAccessSession(req: Request): boolean {
  return verifyAccessToken(readCookie(req.header("cookie"), ACCESS_COOKIE_NAME));
}

function cookieSecure(req: Request): boolean {
  const forwarded = req.header("x-forwarded-proto")?.split(",")[0]?.trim();
  return req.secure || forwarded === "https";
}

export function writeAccessCookie(req: Request, res: Response, token: string): void {
  const secure = cookieSecure(req) ? "; Secure" : "";
  res.setHeader(
    "Set-Cookie",
    `${ACCESS_COOKIE_NAME}=${encodeURIComponent(token)}; HttpOnly; Path=/; SameSite=Lax; Max-Age=${Math.floor(SESSION_TTL_MS / 1000)}${secure}`,
  );
}

export function clearAccessCookie(req: Request, res: Response): void {
  const secure = cookieSecure(req) ? "; Secure" : "";
  res.setHeader(
    "Set-Cookie",
    `${ACCESS_COOKIE_NAME}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0${secure}`,
  );
}

function clientAddress(req: Request): string {
  return req.ip || req.socket.remoteAddress || "unknown";
}

export function recordFailedAccessAttempt(req: Request, now = Date.now()): void {
  const address = clientAddress(req);
  const current = failedAttempts.get(address);
  if (!current || current.resetAt <= now) {
    failedAttempts.set(address, { count: 1, resetAt: now + ATTEMPT_WINDOW_MS });
    return;
  }
  current.count += 1;
}

export function isAccessAttemptLimited(req: Request, now = Date.now()): boolean {
  const current = failedAttempts.get(clientAddress(req));
  return Boolean(current && current.resetAt > now && current.count >= MAX_FAILED_ATTEMPTS);
}

function isPublicAccessPath(path: string): boolean {
  return path === "/api/health"
    || path === "/api/access/status"
    || path === "/api/access/login"
    || path === "/api/access/logout"
    || path.startsWith("/api/auto-director/channel-callbacks/");
}

export function accessGateMiddleware(req: Request, res: Response, next: NextFunction): void {
  if (!isAccessGateEnabled() || isPublicAccessPath(req.path) || requestHasAccessSession(req)) {
    next();
    return;
  }
  res.status(401).json({
    success: false,
    error: "ACCESS_REQUIRED",
    message: "请先输入访问密码。",
  });
}
