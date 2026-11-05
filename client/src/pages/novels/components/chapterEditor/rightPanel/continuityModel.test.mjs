import assert from "node:assert/strict";
import test from "node:test";
import { canReconfirm, factValueText, reconfirmMode, sourceLabel } from "./continuityModel.ts";

test("结构化事实按作者能读的方式展示", () => {
  assert.equal(factValueText({ factType: "life_state", value: "", valueJson: '{"state":"alive"}' }), "存活");
  assert.equal(factValueText({ factType: "life_state", value: "", valueJson: '{"state":"dead"}' }), "死亡");
  assert.equal(factValueText({ factType: "location", value: "", valueJson: '{"location":"山门"}' }), "山门");
  assert.equal(
    factValueText({ factType: "relationship", value: "", valueJson: '{"relation":"ally_of","targetEntityId":"c2"}' }),
    "ally_of · c2",
  );
  assert.equal(
    factValueText({ factType: "relationship", value: "", valueJson: '{"relation":"ally_of","targetEntityId":"c2","active":false}' }),
    "ally_of · c2（已结束）",
  );
  assert.equal(factValueText({ factType: "alias", value: "", valueJson: '{"alias":"林掌门"}' }), "林掌门");
});

test("普通故事事实与损坏 JSON 都原样展示，不丢内容", () => {
  assert.equal(factValueText({ factType: "timeline_event", value: "拜入山门" }), "拜入山门");
  assert.equal(factValueText({ factType: "location", value: "原始值", valueJson: "不是 JSON" }), "原始值");
  assert.equal(factValueText({ factType: "location", value: "原始值", valueJson: '["x"]' }), "原始值");
});

test("来源标签缺失时明确显示不可用", () => {
  assert.equal(sourceLabel({ sourceChapterOrder: 3, sourceChapterTitle: "第三章" }), "第3章 第三章");
  assert.equal(sourceLabel({ sourceChapterOrder: 3 }), "第3章");
  assert.equal(sourceLabel({}), "来源不可用");
});

test("只有来源版本齐备的事实才允许一键重新确认", () => {
  const base = {
    status: "stale",
    valueJson: "{}",
    sourceChapterId: "ch3",
    sourceCurrentVersion: 2,
    entityId: "c1",
  };
  assert.equal(canReconfirm(base), true);
  assert.equal(canReconfirm({ ...base, status: "unavailable" }), false);
  assert.equal(canReconfirm({ ...base, sourceCurrentVersion: null }), false);
  assert.equal(canReconfirm({ ...base, sourceChapterId: null }), false);
  assert.equal(canReconfirm({ ...base, valueJson: null }), false);
  assert.equal(canReconfirm({ ...base, entityId: null }), false);
});

test("重新确认模式区分直接确认、需新证据、需新顺序与不可用", () => {
  const base = {
    status: "stale",
    valueJson: "{}",
    entityId: "c1",
    sourceChapterId: "ch3",
    sourceCurrentVersion: 2,
    evidenceStillPresent: true,
    sourceOrderValid: true,
  };
  assert.equal(reconfirmMode(base), "direct");
  assert.equal(reconfirmMode({ ...base, evidenceStillPresent: false }), "needs_evidence");
  assert.equal(reconfirmMode({ ...base, sourceOrderValid: false }), "needs_order");
  assert.equal(reconfirmMode({ ...base, status: "unavailable" }), "blocked");
  assert.equal(reconfirmMode({ ...base, sourceCurrentVersion: null }), "blocked");
  assert.equal(reconfirmMode({ ...base, entityId: null }), "blocked");
  // 顺序问题优先于证据问题：先让作者把生效章节调整到来源之后。
  assert.equal(
    reconfirmMode({ ...base, sourceOrderValid: false, evidenceStillPresent: false }),
    "needs_order",
  );
});
