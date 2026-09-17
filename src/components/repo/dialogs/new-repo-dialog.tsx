import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import type { LicenseTemplateInfo } from "@/lib/backend/protocol";
import {
    gitignoreTemplatesQuery,
    licensesQuery,
} from "@/lib/backend/queries/template-queries";
import { createRepositoryOnDisk } from "@/lib/repositories/create-repository";
import {
    lastRepositoryDirectory,
    rememberRepositoryDirectory,
} from "@/lib/repositories/destination-memory";

import { Button } from "../../ui/button";
import { Checkbox } from "../../ui/checkbox";
import {
    Dialog,
    DialogClose,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
} from "../../ui/dialog";
import { Field, FieldLabel } from "../../ui/field";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "../../ui/select";
import { Tabs, TabsList, TabsTrigger } from "../../ui/tabs";
import { DestinationInput } from "./destination-input";

const NEW_FOLDER = "new-folder";
const EXISTING_FOLDER = "existing-folder";
const NO_GITIGNORE = "";
const NO_LICENSE = "";

const NO_LICENSE_OPTION: LicenseTemplateInfo = {
    id: NO_LICENSE,
    name: "None",
    description: "No license file",
};

/**
 * Creates an empty repository with optional scaffolding in a new folder
 * under the chosen parent. Files are written but not committed.
 */
export function NewRepoDialog({
    open,
    onClose,
    onCreated,
}: {
    open: boolean;
    onClose: () => void;
    onCreated: (repoPath: string) => void;
}) {
    const { backend } = useAppServices();

    const [mode, setMode] = useState(NEW_FOLDER);
    const [parentDirectory, setParentDirectory] = useState(
        lastRepositoryDirectory
    );
    const [folderName, setFolderName] = useState("");
    const [readme, setReadme] = useState(true);
    const [gitignore, setGitignore] = useState(NO_GITIGNORE);
    const [license, setLicense] = useState(NO_LICENSE);
    const [error, setError] = useState<string | null>(null);
    const [submitting, setSubmitting] = useState(false);

    // Fresh form for the next open; re-reads the remembered destination so
    // a clone run since last time is picked up.
    const resetForm = () => {
        setMode(NEW_FOLDER);
        setParentDirectory(lastRepositoryDirectory());
        setFolderName("");
        setReadme(true);
        setGitignore(NO_GITIGNORE);
        setLicense(NO_LICENSE);
        setError(null);
    };

    const handleClose = () => {
        resetForm();
        onClose();
    };

    const templates = useQuery(gitignoreTemplatesQuery({ backend }));
    const licenses = useQuery(licensesQuery({ backend }));

    const gitignoreOptions = [
        { label: "None", value: NO_GITIGNORE },
        ...(templates.data ?? []).map((template) => ({
            label: template.label,
            value: template.id,
        })),
    ];

    const licenseOptions: LicenseTemplateInfo[] = [
        NO_LICENSE_OPTION,
        ...(licenses.data ?? []),
    ];
    const licenseItems = licenseOptions.map((item) => ({
        value: item.id,
        label: (
            <span className="flex flex-col">
                <span className="truncate">{item.name}</span>
                <span className="truncate text-xs text-muted-foreground">
                    {item.description}
                </span>
            </span>
        ),
    }));

    const inPlace = mode === EXISTING_FOLDER;
    const trimmedName = folderName.trim();
    const canCreate =
        !!parentDirectory && (inPlace || !!trimmedName) && !submitting;

    const handleCreate = async () => {
        if (!canCreate) return;
        setError(null);
        setSubmitting(true);
        try {
            const outcome = await createRepositoryOnDisk(backend, {
                parentDirectory,
                name: inPlace ? "" : trimmedName,
                initInPlace: inPlace,
                readme,
                gitignoreTemplate: gitignore || null,
                license: license || null,
            });
            switch (outcome.status) {
                case "failed":
                    setError(outcome.error.message);
                    return;
                case "created":
                    rememberRepositoryDirectory(parentDirectory);
                    resetForm();
                    onCreated(outcome.repo.repoPath);
                    return;
            }
        } finally {
            setSubmitting(false);
        }
    };

    return (
        <Dialog open={open} onOpenChange={(next) => !next && handleClose()}>
            <DialogPopup data-testid="new-repo-dialog">
                <DialogHeader>
                    <DialogTitle>Create new repo</DialogTitle>
                    <DialogDescription>
                        {inPlace
                            ? "Initializes an empty repository directly in the chosen folder. Existing files are kept."
                            : "Initializes an empty repository in a new folder. Scaffolding files are written but not committed."}
                    </DialogDescription>
                </DialogHeader>
                <DialogPanel className="flex flex-col gap-3">
                    <Tabs
                        value={mode}
                        onValueChange={(value) => {
                            if (
                                value === NEW_FOLDER ||
                                value === EXISTING_FOLDER
                            )
                                setMode(value);
                        }}
                    >
                        <TabsList>
                            <TabsTrigger value={NEW_FOLDER}>
                                New folder
                            </TabsTrigger>
                            <TabsTrigger value={EXISTING_FOLDER}>
                                Existing folder
                            </TabsTrigger>
                        </TabsList>
                    </Tabs>
                    <Field>
                        <FieldLabel>
                            {inPlace ? "Folder to initialize" : "New folder"}
                        </FieldLabel>
                        <DestinationInput
                            actionLabel={
                                inPlace ? "Will initialize" : "Will create into"
                            }
                            folderPlaceholder={
                                inPlace ? "(this folder)" : "my-project"
                            }
                            parentDirectory={parentDirectory}
                            folderName={inPlace ? "" : folderName}
                            folderNameDisabled={inPlace}
                            autoFocus
                            onChangeParentDirectory={setParentDirectory}
                            onChangeFolderName={setFolderName}
                        />
                    </Field>
                    <Field>
                        <FieldLabel>Git ignore</FieldLabel>
                        <Select
                            value={gitignore}
                            onValueChange={(value) => {
                                if (typeof value === "string")
                                    setGitignore(value);
                            }}
                            items={gitignoreOptions}
                        >
                            <SelectTrigger
                                className="w-full"
                                aria-label="Git ignore template"
                            >
                                <SelectValue />
                            </SelectTrigger>
                            <SelectPopup>
                                {gitignoreOptions.map(({ label, value }) => (
                                    <SelectItem key={value} value={value}>
                                        {label}
                                    </SelectItem>
                                ))}
                            </SelectPopup>
                        </Select>
                    </Field>
                    <Field>
                        <FieldLabel>License</FieldLabel>
                        <Select
                            aria-label="Select license"
                            value={license}
                            onValueChange={(value) => {
                                if (typeof value === "string")
                                    setLicense(value);
                            }}
                            items={licenseItems}
                        >
                            <SelectTrigger
                                className="w-full py-1"
                                aria-label="License"
                            >
                                <SelectValue />
                            </SelectTrigger>
                            <SelectPopup>
                                {licenseItems.map(({ label, value }) => (
                                    <SelectItem key={value} value={value}>
                                        {label}
                                    </SelectItem>
                                ))}
                            </SelectPopup>
                        </Select>
                    </Field>
                    <label className="flex cursor-pointer items-center gap-2 text-sm">
                        <Checkbox
                            checked={readme}
                            onCheckedChange={(checked) =>
                                setReadme(checked === true)
                            }
                        />
                        Add a README
                    </label>
                    {error ? (
                        <p className="text-sm text-destructive" role="alert">
                            {error}
                        </p>
                    ) : null}
                </DialogPanel>
                <DialogFooter>
                    <DialogClose render={<Button variant="ghost" />}>
                        Cancel
                    </DialogClose>
                    <Button
                        disabled={!canCreate}
                        loading={submitting}
                        data-testid="new-repo-submit"
                        onClick={() => void handleCreate()}
                    >
                        Create
                    </Button>
                </DialogFooter>
            </DialogPopup>
        </Dialog>
    );
}
