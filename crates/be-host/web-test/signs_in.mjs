// The web host signs in and opens the workspace.
export default async function (test) {
    const tree = await test.drive("tree");
    if (!tree.includes('TreeItem "My files"')) throw new Error(`no My files in\n${tree}`);
}
