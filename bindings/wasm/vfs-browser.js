class VFS {
    constructor() {
	this.dbName = 'libsql-vfs';
        this.openFiles = new Map();
        this.nextFd = 1;
        this.ready = this.initializeDB();
        console.log('VFS constructed');
    }

    async initializeDB() {
        return new Promise((resolve, reject) => {
            const request = indexedDB.open(this.dbName, 1);

            request.onerror = () => reject(request.error);

            request.onupgradeneeded = (event) => {
                const db = event.target.result;
                // Store for file metadata
                db.createObjectStore('files', { keyPath: 'name' });
                // Store for actual file contents (pages)
                db.createObjectStore('pages');
            };

            request.onsuccess = () => {
                this.db = request.result;
                resolve();
            };
        });
    }

    async open(path, flags) {
        await this.ready;

        const fd = this.nextFd++;
        this.openFiles.set(fd, { path, flags });

        // Ensure we have metadata for this file
        const tx = this.db.transaction('files', 'readwrite');
        const store = tx.objectStore('files');

        const metadata = await new Promise((resolve) => {
            const req = store.get(path);
            req.onsuccess = () => resolve(req.result);
        });

        if (!metadata) {
            await new Promise((resolve) => {
                const req = store.put({ name: path, size: BigInt(0) });
                req.onsuccess = () => resolve();
            });
        }

        return fd;
    }

    async pread(fd, buffer, offset) {
        await this.ready;
        const fileInfo = this.openFiles.get(fd);
        if (!fileInfo) throw new Error('Invalid file descriptor');

        const tx = this.db.transaction('pages', 'readonly');
        const store = tx.objectStore('pages');

        const pageKey = `${fileInfo.path}:${offset}`;
        const data = await new Promise((resolve) => {
            const req = store.get(pageKey);
            req.onsuccess = () => resolve(req.result);
        });

        if (data) {
            buffer.set(new Uint8Array(data));
            return data.byteLength;
        }
        return 0;
    }
	   async pwrite(fd, buffer, offset) {
        await this.ready;
        const fileInfo = this.openFiles.get(fd);
        if (!fileInfo) throw new Error('Invalid file descriptor');

        const tx = this.db.transaction(['pages', 'files'], 'readwrite');
        const pageStore = tx.objectStore('pages');
        const fileStore = tx.objectStore('files');

        const pageKey = `${fileInfo.path}:${offset}`;

        await Promise.all([
            new Promise((resolve) => {
                const req = pageStore.put(buffer, pageKey);
                req.onsuccess = () => resolve();
            }),
            new Promise((resolve) => {
                const req = fileStore.get(fileInfo.path);
                req.onsuccess = () => {
                    const metadata = req.result || { name: fileInfo.path, size: BigInt(0) };
                    const newSize = offset + buffer.length;
                    if (newSize > metadata.size) {
                        metadata.size = BigInt(newSize);
                        fileStore.put(metadata);
                    }
                    resolve();
                };
            })
        ]);

        return buffer.length;
    }

    async size(fd) {
        await this.ready;
        const fileInfo = this.openFiles.get(fd);
        if (!fileInfo) throw new Error('Invalid file descriptor');

        const tx = this.db.transaction('files', 'readonly');
        const store = tx.objectStore('files');

        const metadata = await new Promise((resolve) => {
            const req = store.get(fileInfo.path);
            req.onsuccess = () => resolve(req.result);
        });

        return metadata ? metadata.size : BigInt(0);
    }

    sync(fd) {
        // IndexedDB transactions are automatically committed
        return Promise.resolve();
    }
}

export { VFS };
