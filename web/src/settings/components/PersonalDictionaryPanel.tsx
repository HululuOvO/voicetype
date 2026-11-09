import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  exportPersonalDictionary,
  importPersonalDictionary,
  loadPersonalDictionary,
  type PersonalDictionary,
  savePersonalDictionary,
} from "@/settings/bridge";
import { Button } from "@/settings/components/Button";
import { Textarea } from "@/settings/components/Textarea";
import { Section, SectionContent, SectionHeader } from "@/settings/layout/PageLayout";

const lines = (value: string) =>
  value.split("\n").map((line) => line.trim()).filter(Boolean);

export function PersonalDictionaryPanel() {
  const { t } = useTranslation();
  const [words, setWords] = useState("");
  const [rules, setRules] = useState("");
  const [busy, setBusy] = useState(true);
  const [ready, setReady] = useState(false);
  const [status, setStatus] = useState("");

  useEffect(() => {
    let active = true;
    loadPersonalDictionary()
      .then((data) => {
        if (!active) return;
        setWords(data.hotwords.join("\n"));
        setRules(data.replacements.map(({ from, to }) => `${from} → ${to}`).join("\n"));
        setReady(true);
      })
      .catch((error: unknown) => {
        if (active) setStatus(String(error));
      })
      .finally(() => {
        if (active) setBusy(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const readDraft = (): PersonalDictionary | null => {
    const replacements: PersonalDictionary["replacements"] = [];
    for (const [index, line] of rules.split("\n").entries()) {
      if (!line.trim()) continue;
      const arrow = line.indexOf("→");
      const from = line.slice(0, arrow).trim();
      const to = line.slice(arrow + 1).trim();
      if (arrow < 0 || !from || !to) {
        setStatus(t("ui.personal_dictionary.bad_rule", { line: index + 1 }));
        return null;
      }
      replacements.push({ from, to });
    }
    return { hotwords: lines(words), replacements };
  };

  const persist = async (backup: boolean) => {
    const data = readDraft();
    if (!data) return;
    setBusy(true);
    setStatus("");
    try {
      if (backup) {
        if (await exportPersonalDictionary(data)) setStatus(t("ui.personal_dictionary.exported"));
      } else {
        await savePersonalDictionary(data);
        setStatus(t("ui.personal_dictionary.saved"));
      }
    } catch (error) {
      setStatus(`${t("ui.personal_dictionary.failed")} ${String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const readBackup = async () => {
    setBusy(true);
    setStatus("");
    try {
      const data = await importPersonalDictionary();
      if (data) {
        setWords(data.hotwords.join("\n"));
        setRules(data.replacements.map(({ from, to }) => `${from} → ${to}`).join("\n"));
        setReady(true);
        setStatus(t("ui.personal_dictionary.imported"));
      }
    } catch (error) {
      setStatus(`${t("ui.personal_dictionary.failed")} ${String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section>
      <SectionHeader title={t("ui.personal_dictionary.title")} />
      <SectionContent>
        <div className="space-y-3 p-3">
          <p className="text-sm text-text-muted">{t("ui.personal_dictionary.description")}</p>
          <label className="block text-sm" htmlFor="personal-hotwords">
            {t("ui.personal_dictionary.hotwords")}
          </label>
          <Textarea id="personal-hotwords" value={words} onChange={setWords} disabled={busy || !ready}
            placeholder={"Obsidian\nQwen"} />
          <label className="block text-sm" htmlFor="personal-replacements">
            {t("ui.personal_dictionary.replacements")}
          </label>
          <Textarea id="personal-replacements" value={rules} onChange={setRules} disabled={busy || !ready}
            placeholder="obsidian → Obsidian" />
          <div className="flex flex-wrap gap-2">
            <Button variant="accent" disabled={busy || !ready} onClick={() => void persist(false)}>
              {t("ui.personal_dictionary.save")}
            </Button>
            <Button disabled={busy} onClick={() => void readBackup()}>{t("ui.personal_dictionary.import")}</Button>
            <Button disabled={busy || !ready} onClick={() => void persist(true)}>{t("ui.personal_dictionary.export")}</Button>
          </div>
          <p role="status" className="text-sm text-text-muted">{status}</p>
        </div>
      </SectionContent>
    </Section>
  );
}
