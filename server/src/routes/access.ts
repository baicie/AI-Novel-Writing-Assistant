import { Router } from "express";
import { z } from "zod";
import type { ApiResponse } from "@ai-novel/shared/types/api";
import {
  accessPasswordMatches,
  clearAccessCookie,
  createAccessToken,
  isAccessAttemptLimited,
  isAccessGateEnabled,
  recordFailedAccessAttempt,
  requestHasAccessSession,
  writeAccessCookie,
} from "../platform/access/accessSession";
import { validate } from "../middleware/validate";

const router = Router();

const loginSchema = z.object({
  password: z.string().min(1, "请输入访问密码。"),
});

router.get("/status", (req, res) => {
  const required = isAccessGateEnabled();
  const response: ApiResponse<{ required: boolean; authenticated: boolean }> = {
    success: true,
    data: {
      required,
      authenticated: !required || requestHasAccessSession(req),
    },
  };
  res.status(200).json(response);
});

router.post("/login", validate({ body: loginSchema }), (req, res) => {
  if (!isAccessGateEnabled()) {
    const response: ApiResponse<{ authenticated: boolean }> = {
      success: true,
      data: { authenticated: true },
      message: "这台服务没有启用访问密码。",
    };
    res.status(200).json(response);
    return;
  }
  if (isAccessAttemptLimited(req)) {
    const response: ApiResponse<null> = {
      success: false,
      error: "尝试次数过多，请稍后再试。",
    };
    res.status(429).json(response);
    return;
  }
  const { password } = req.body as z.infer<typeof loginSchema>;
  if (!accessPasswordMatches(password)) {
    recordFailedAccessAttempt(req);
    const response: ApiResponse<null> = {
      success: false,
      error: "密码不正确。",
    };
    res.status(401).json(response);
    return;
  }
  writeAccessCookie(req, res, createAccessToken());
  const response: ApiResponse<{ authenticated: boolean }> = {
    success: true,
    data: { authenticated: true },
    message: "已进入工作台。",
  };
  res.status(200).json(response);
});

router.post("/logout", (req, res) => {
  clearAccessCookie(req, res);
  const response: ApiResponse<{ authenticated: boolean }> = {
    success: true,
    data: { authenticated: false },
    message: "已退出。",
  };
  res.status(200).json(response);
});

export default router;
