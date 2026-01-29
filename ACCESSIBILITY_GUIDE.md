# GroqBara Accessibility 權限設定指南

## 問題症狀
- "Open Accessibility Settings" 按鈕沒反應
- 無法將 app 加入 Accessibility 列表
- Fn 鍵監聽無法啟動

## 根本原因
macOS 的 Accessibility 權限機制：
1. App 必須先被**啟動並嘗試使用** Accessibility API
2. 系統才會自動將其加入可授權列表
3. 單純從 System Settings 手動添加不一定有效

## 解決方案

### ⚠️ 重要：授予權限後必須重啟 App

macOS 的 Accessibility 權限機制要求：
1. 授予權限後，必須**完全關閉 app**（Cmd+Q 或右鍵 Quit）
2. 重新開啟 app
3. 權限才會真正生效

**常見錯誤**：在授予權限後直接嘗試啟用 Fn key → 會失敗並顯示 "Failed to create event tap"

### 正確流程（推薦）
### 正確流程（推薦）

**首次設定：**
1. 開啟 GroqBara
2. 勾選「Use Fn key instead」
3. 如果顯示錯誤，點擊「Trigger Permission Dialog」
4. 系統會跳出權限請求 → 點擊「Open System Settings」
5. 在 Accessibility 列表中勾選「Groq Transcriber」
6. **⚠️ 重要：完全關閉 app（Cmd+Q）**
7. 重新開啟 app
8. 再次勾選「Use Fn key instead」→ 應該會成功！

**後續使用：**
- 權限已授予且 app 已重啟過一次後，Fn key 功能就會一直可用
- 不需要每次都重啟

### 方法 2：自動觸發（如果方法 1 沒用）
1. 開啟 GroqBara
2. 勾選「Use Fn key instead」
3. 點擊「Trigger Permission Dialog」按鈕
4. 系統應該會跳出權限請求對話框
5. 點擊「Open System Settings」
6. 在 Accessibility 列表中找到並勾選「Groq Transcriber」
7. **⚠️ 完全關閉 app（Cmd+Q）並重新開啟**
8. 勾選「Use Fn key instead」

### 方法 3：手動設定
### 方法 3：手動設定
1. 開啟 **System Settings** (系統設定)
2. 進入 **Privacy & Security** (隱私權與安全性)
3. 選擇左側的 **Accessibility** (輔助使用)
4. 點擊下方的鎖頭圖示解鎖（需要輸入密碼）
5. 點擊 **+** 號
6. 找到並選擇：`/Applications/Groq Transcriber.app`（或你放置的位置）
7. 確保該 app 旁邊的開關是**開啟**狀態
8. **⚠️ 完全關閉 app（Cmd+Q）並重新開啟**

### 方法 4：終端命令強制授權（進階）
```bash
# 顯示目前有 Accessibility 權限的 app
sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db "SELECT client FROM access WHERE service='kTCCServiceAccessibility'"

# 如果 Groq Transcriber 不在列表中，嘗試：
# 1. 先完全關閉 app
killall "Groq Transcriber"

# 2. 重新開啟並立即嘗試啟用 Fn key
# 這會觸發系統的權限請求
```

## 測試步驟
1. 啟用「Use Fn key instead」
2. 檢查 Logs 區域是否有錯誤訊息
3. 如果顯示「Accessibility permission granted」→ 成功！
4. 按住 Fn 鍵測試是否開始錄音

## 常見問題

### Q: 我已經授予權限了，但還是顯示 "Failed to create event tap"
**A**: 這是最常見的問題！解決方式：
1. **完全關閉 app**（不是最小化，是 Cmd+Q 或右鍵 Quit）
2. 重新開啟 app
3. 再次嘗試啟用 Fn key

macOS 要求 app 重啟後權限才會生效。

### Q: 沒有跳出權限對話框
**A**: macOS Sequoia 有時會靜默拒絕。試試：
1. 檢查 System Settings → Privacy & Security 是否有待處理的請求
2. 完全重新安裝 app（拖到垃圾桶，清空，重新解壓安裝）
3. 第一次開啟時立即嘗試啟用 Fn key

### Q: Fn 鍵還是沒反應
**A**: 確認：
1. Accessibility 權限已授予
2. 「Use Fn key instead」已勾選
3. Trigger mode 是 hold 或 toggle
4. 檢查 Logs 是否顯示「Fn key listening enabled」

## 技術細節
- Fn 鍵監聽使用 `CGEventTap` API
- 需要 `kCGSessionEventTap` 權限
- macOS 會檢查 app 是否在 Accessibility 白名單中
- 權限資料庫位於：`~/Library/Application Support/com.apple.TCC/TCC.db`

## 更新記錄
- **v0.1.1** (2026-01-29)
  - 新增「Trigger Permission Dialog」按鈕
  - 改進權限檢查邏輯
  - 更清楚的錯誤提示
