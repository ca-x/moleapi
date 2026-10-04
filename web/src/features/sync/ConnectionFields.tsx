import { t, useLanguage } from "../../shared/i18n";
import { TextField } from "@radix-ui/themes";
import { Field } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
export default function ConnectionFields() {
  useLanguage();
  const {
    serverUrl,
    setServerUrl,
    serverUser,
    setServerUser,
    serverPassword,
    setServerPassword,
  } = useWorkbench();
  return (
    <>
      <Field label={t("服务端地址")}>
        <TextField.Root
          required
          autoFocus
          type="url"
          placeholder="https://mole.example.com"
          value={serverUrl}
          onChange={(e) => setServerUrl(e.target.value)}
        />
      </Field>
      <Field label={t("用户名")}>
        <TextField.Root
          required
          autoComplete="username"
          value={serverUser}
          onChange={(e) => setServerUser(e.target.value)}
        />
      </Field>
      <Field label={t("密码")}>
        <TextField.Root
          required
          autoComplete="current-password"
          type="password"
          value={serverPassword}
          onChange={(e) => setServerPassword(e.target.value)}
        />
      </Field>
    </>
  );
}
