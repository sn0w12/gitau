import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { ChevronsUpDown } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
    InsetInput,
    InsetInputAddition,
    InsetInputInput,
} from "@/components/ui/inset-input";

export function displayDist(dist: string) {
    return dist.replaceAll("\\", "/");
}

/**
 * Destination picker shared by repository creation and cloning: a parent
 * folder chosen through the OS dialog plus the new folder name. The folder
 * name input sits inside the inset so the composed path reads as one unit.
 */
export function DestinationInput({
    parentDirectory,
    folderName,
    parentTitle = "Choose a location",
    folderPlaceholder,
    autoFocus = false,
    onChangeParentDirectory,
    onChangeFolderName,
}: {
    parentDirectory: string;
    folderName: string;
    parentTitle?: string;
    folderPlaceholder: string;
    autoFocus?: boolean;
    onChangeParentDirectory: (directory: string) => void;
    onChangeFolderName: (name: string) => void;
}) {
    const pickFolder = async () => {
        const selection = await openDirectoryDialog({
            directory: true,
            multiple: false,
            title: parentTitle,
        });
        if (typeof selection === "string" && selection.length > 0) {
            onChangeParentDirectory(selection);
        }
    };

    return (
        <>
            <InsetInput>
                <InsetInputAddition align="inline-start">
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="px-1"
                        onClick={() => void pickFolder()}
                    >
                        {parentDirectory
                            ? `${displayDist(parentDirectory)}/`
                            : "Browse"}
                        {!parentDirectory && <ChevronsUpDown />}
                    </Button>
                </InsetInputAddition>
                <InsetInputInput
                    type="text"
                    placeholder={folderPlaceholder}
                    value={folderName}
                    autoFocus={autoFocus}
                    onChange={(event) => onChangeFolderName(event.target.value)}
                />
            </InsetInput>
        </>
    );
}
