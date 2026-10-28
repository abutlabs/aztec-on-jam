// Imported first by the scripts here: Aztec's packages need Node 22+ (the PXE uses
// Set.prototype.intersection), and fail deep inside a transaction on anything older.
const major = Number(process.versions.node.split('.')[0]);
if (major < 22) {
  console.error(`Node 22+ is needed (this is Node ${process.versions.node}). With nvm: nvm install 22 && nvm use 22`);
  process.exit(2);
}
