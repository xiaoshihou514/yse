<template>
  <div class="welcome">
    <div class="welcome-card">
      <div class="welcome-logo">
        <img src="/icon.png" alt="盐水鹅" class="welcome-img" />
      </div>
      <h1 class="welcome-title">盐水鹅</h1>
      <p class="welcome-sub">邮件即聊天 · 随时随地开发</p>
      <div class="welcome-steps">
        <div class="step">
          <span class="step-num">1</span>
          <div class="step-body">
            <span class="step-title">配置邮箱</span>
            <span class="step-desc">添加你的 IMAP/SMTP 邮箱账户</span>
          </div>
        </div>
        <div class="step">
          <span class="step-num">2</span>
          <div class="step-body">
            <span class="step-title">添加插件</span>
            <span class="step-desc">安装 opencode-bot、file-tree 等插件</span>
          </div>
        </div>
        <div class="step">
          <span class="step-num">3</span>
          <div class="step-body">
            <span class="step-title">开始聊天</span>
            <span class="step-desc">发送消息给联系人，或通过手机控制电脑</span>
          </div>
        </div>
      </div>
      <t-button
        theme="primary"
        size="large"
        class="welcome-btn"
        @click="$router.push('/config')"
        >开始配置</t-button
      >
      <p class="welcome-footer">本机主机名: {{ hostname }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useYseStore } from "@/stores/yse";

const store = useYseStore();
const hostname = store.localHostname || "加载中...";
</script>

<style scoped>
.welcome {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  box-sizing: border-box;
  background: var(--td-bg-color-page);
}
.welcome-card {
  max-width: 400px;
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 20px;
}
.welcome-logo {
  width: 72px;
  height: 72px;
  border-radius: 18px;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
}
.welcome-img {
  width: 48px;
  height: 48px;
}
.welcome-title {
  font-size: 28px;
  font-weight: 700;
  margin: 0;
  color: var(--td-text-color-primary);
}
.welcome-sub {
  font-size: 14px;
  color: var(--td-text-color-placeholder);
  margin: -8px 0 0;
}
.welcome-steps {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.step {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 14px;
  border-radius: 10px;
  background: var(--td-bg-color-secondarycontainer);
}
.step-num {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--td-brand-color);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 600;
  flex-shrink: 0;
}
.step-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.step-title {
  font-size: 15px;
  font-weight: 500;
  color: var(--td-text-color-primary);
}
.step-desc {
  font-size: 13px;
  color: var(--td-text-color-placeholder);
}
.welcome-btn {
  width: 100%;
}
.welcome-footer {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  margin: 0;
}
</style>
