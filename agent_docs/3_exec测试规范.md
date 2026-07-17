请按照以下步骤依次执行命令，验证新版 `exec` 工具是否按预期工作。  
> 每执行一步，请根据返回结果判断测试是否通过，并记录。  
> 测试结束后，汇总一份通过/失败的报告。  
> 环境说明：本地有 Docker，可运行容器；SSH 客户端已安装；`exec` 工具支持 `server` 参数，格式为 `user@host[:port]`。

---

### 1. 短命令（本地）
- 调用 `exec({ command: "echo hello" })`
- 预期：返回 `status: "completed"`，`output` 包含 `hello`。
- 判定：`status` 为 `completed` 且 `output` 含 `hello` → 通过。

### 2. 长命令超时降级（本地）
- 调用 `exec({ command: "sleep 130" })`（约2分10秒）
- 预期：约2分钟后返回 `status: "running"`，包含 `task_id`（如 `task_1`）。
- 记录下 `task_id` 的值。
- 判定：`status` 为 `running` 且 `task_id` 非空 → 通过。

### 3. 主窗格未被阻塞
- 立即调用 `exec({ command: "echo main still works" })`
- 预期：快速返回 `status: "completed"`，`output` 包含 `main still works`。
- 判定：结果如上 → 通过。

### 4. 查询后台任务（使用上一步的 `task_id`）
- 调用 `exec({ task_id: "<第2步得到的task_id>" })`
- 预期：阻塞等待直到 sleep 结束，最终返回 `status: "completed"`，`output` 可能为空。
- 判定：`status` 为 `completed` → 通过。

### 5. 无参数自动查询最近后台任务
- 先执行长命令 `exec({ command: "sleep 120" })`，得到 `task_id`（例如 `task_2`）。
- 然后**不记录该 `task_id`**，直接调用 `exec()`（无参数）
- 预期：工具自动查询最近后台任务（即这个 sleep 120），最终返回 `completed`。
- 判定：`status` 为 `completed` → 通过。

### 6. 查询不存在的任务
- 调用 `exec({ task_id: "task_999" })`
- 预期：返回错误信息，如“任务不存在或已被清理”。
- 判定：返回包含错误文字 → 通过。

### 7. 同时指定 `command` 和 `task_id`（应报错）
- 调用 `exec({ command: "echo hello", task_id: "task_1" })`
- 预期：返回错误，如“不能同时指定 command 和 task_id”。
- 判定：返回错误 → 通过。

### 8. 工作目录测试
- 调用 `exec({ command: "pwd", directory: "/tmp" })`
- 预期：`output` 为 `/tmp`。
- 判定：输出为 `/tmp` → 通过。

---

### 9. 准备远程测试环境
执行以下命令（均通过 `exec` 工具）：

1. 生成 SSH 密钥对（如果不存在）：  
   `exec({ command: "mkdir -p ~/.ssh && ssh-keygen -t rsa -f ~/.ssh/id_rsa -N '' -q" })`

2. 启动带有 SSH 服务的 Docker 容器（使用 host 网络模式，避免端口问题）：  
   `exec({ command: "docker run -d --network host --name test-sshd rastasheep/ubuntu-sshd:latest" })`  
   （如果需要，先拉取镜像：`docker pull rastasheep/ubuntu-sshd:latest`）

3. 等待容器就绪（2秒）：  
   `exec({ command: "sleep 2" })`

4. 将本地公钥注入容器：  
   `exec({ command: "docker exec test-sshd bash -c 'mkdir -p /root/.ssh && cat >> /root/.ssh/authorized_keys' < ~/.ssh/id_rsa.pub" })`  
   （注意：exec 不支持重定向，这里需要调整命令。可改用管道：  
   `exec({ command: "cat ~/.ssh/id_rsa.pub | docker exec -i test-sshd bash -c 'mkdir -p /root/.ssh && cat >> /root/.ssh/authorized_keys'" })`）

5. 验证 SSH 连接：  
   `exec({ command: "echo connected", server: "root@localhost" })`  
   预期返回 `completed`，输出 `connected`。  
   若失败，检查原因并重试。

---

### 10. 远程短命令
- 调用 `exec({ command: "echo remote hello", server: "root@localhost" })`
- 预期：返回 `completed`，输出 `remote hello`。
- 判定：通过。

### 11. 远程长命令超时降级
- 调用 `exec({ command: "sleep 130", server: "root@localhost" })`
- 预期：约2分钟后返回 `running`，包含 `task_id`（如 `task_1`，注意与本地任务无关）。
- 记录 `task_id`。
- 判定：`status` 为 `running` → 通过。

### 12. 远程后台查询
- 调用 `exec({ task_id: "<远程task_id>" })`
- 预期：等待后返回 `completed`。
- 判定：通过。

### 13. 远程主窗格未阻塞
- 立即执行 `exec({ command: "echo remote after", server: "root@localhost" })`
- 预期：快速返回 `completed`。
- 判定：通过。

### 14. 本地与远程多任务独立性
- 本地再执行一个长命令：`exec({ command: "sleep 180" })`，得到本地新 `task_id`（如 `task_3`）。
- 远程执行一个长命令：`exec({ command: "sleep 150", server: "root@localhost" })`，得到远程新 `task_id`（如 `task_2`）。
- 分别查询两个任务：先查本地 `task_3`，再查远程 `task_2`，都应正常完成。
- 判定：两个任务各自独立完成，不相互干扰 → 通过。

### 15. 特殊字符命令
- 调用 `exec({ command: "echo \"it's a test\"" })`  
  （注意转义，实际传给工具的命令字符串应包含双引号包裹的单引号，或使用单引号包裹字符串。工具会自动处理。）
- 预期：输出 `it's a test`。
- 判定：通过。

### 16. 命令执行失败（非零退出码）
- 调用 `exec({ command: "ls /nonexistent" })`
- 预期：返回 `status: "completed"`，`output` 包含错误信息（如 `No such file or directory`）。
- 判定：包含错误信息 → 通过（工具不拒绝失败命令）。

### 17. 多个后台任务管理
- 依次启动两个本地长命令（不等待）：
  - 先执行 `exec({ command: "sleep 120" })`，得到 `task_4`
  - 再执行 `exec({ command: "sleep 180" })`，得到 `task_5`（注意第二个调用时会因为第一个已降级而主窗格空闲，所以会直接开始第二个长命令，也会超时降级）
- 然后先查询 `task_5`（更晚完成），再查询 `task_4`，确保都能正确获取结果。
- 判定：两个任务都返回 `completed` → 通过。

---

### 18. 清理环境
- 停止并删除测试容器：  
  `exec({ command: "docker rm -f test-sshd" })`

---

## 测试报告要求
全部步骤执行后，请给出每个测试项的结果（通过/失败），以及任何异常现象。
