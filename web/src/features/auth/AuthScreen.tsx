import { errorCopy } from "../../shared/i18n/errors";
import type { ErrorCopy } from "../../shared/i18n/errors";
import LanguageSelector from "../../shared/i18n/LanguageSelector";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { useState } from "react";
import {
  Button,
  Callout,
  Card,
  Flex,
  Heading,
  Text,
  TextField,
} from "@radix-ui/themes";
import { ArrowRight, KeyRound, Server } from "lucide-react";
import { api } from "../../shared/api";
import { Field } from "../../shared/ui";
import type { AuthStatus } from "../../shared/types";

export default function AuthScreen({
  status,
  onLogin,
}: {
  status: AuthStatus;
  onLogin: () => void;
}) {
  useLanguage();
  const [register, setRegister] = useState(status.setup_required);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [setupToken, setSetupToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorCopy>("");
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const result = await api<{ token: string; username: string }>(
        register ? "/api/auth/register" : "/api/auth/login",
        "POST",
        { username, password, setup_token: setupToken },
      );
      sessionStorage.setItem("moleapi_token", result.token);
      sessionStorage.setItem("moleapi_username", result.username);
      onLogin();
    } catch (error) {
      setError(errorCopy(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <main className="auth-screen">
      <div className="auth-brand">
        <img src="/logo.png" width="132" height="132" alt={t("MoleAPI 鼹鼠 Logo")} />
        <Heading size="8">MoleAPI</Heading>
        <Text color="gray">{t("你的 API，你的工作台。")}</Text>
      </div>
      <Card size="4" className="auth-card">
        <form onSubmit={submit}>
          <Flex direction="column" gap="5">
            <LanguageSelector />
            <div>
              <Heading size="5">
                {status.setup_required
                  ? t("创建管理员账户")
                  : register
                    ? t("创建账户")
                    : t("登录工作台")}
              </Heading>
              <Text as="p" size="2" color="gray">
                {status.setup_required
                  ? t("使用服务端首次启动时显示的初始化令牌。")
                  : t("登录你的自托管服务，访问项目和团队资源。")}
              </Text>
            </div>
            <Field label={t("用户名")}>
              <TextField.Root
                required
                minLength={3}
                maxLength={64}
                autoFocus
                autoComplete="username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
              />
            </Field>
            <Field
              label={t("密码")}
              hint={register ? t("至少 10 个字符，可使用密码管理器。") : undefined}
            >
              <TextField.Root
                required
                minLength={register ? 10 : undefined}
                type="password"
                autoComplete={register ? "new-password" : "current-password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </Field>
            {status.setup_required && (
              <Field label={t("初始化令牌")}>
                <TextField.Root
                  required
                  type="password"
                  autoComplete="off"
                  value={setupToken}
                  onChange={(e) => setSetupToken(e.target.value)}
                >
                  <TextField.Slot>
                    <KeyRound size={15} />
                  </TextField.Slot>
                </TextField.Root>
              </Field>
            )}
            {error && (
              <Callout.Root color="red" role="alert">
                <Callout.Text>{translateCopy(error)}</Callout.Text>
              </Callout.Root>
            )}
            <Button size="3" type="submit" loading={busy}>
              {register ? t("创建并登录") : t("登录")}
              <ArrowRight size={16} />
            </Button>
            {!status.setup_required && status.registration_enabled && (
              <Button
                variant="ghost"
                color="gray"
                type="button"
                onClick={() => {
                  setRegister(!register);
                  setError("");
                }}
              >
                {register ? t("已有账户，返回登录") : t("没有账户？注册")}
              </Button>
            )}
          </Flex>
        </form>
      </Card>
      <Flex align="center" gap="2" className="auth-caption">
        <Server size={14} />
        <Text size="1" color="gray"> {t("数据保存在你自己的服务器")} </Text>
      </Flex>
    </main>
  );
}
