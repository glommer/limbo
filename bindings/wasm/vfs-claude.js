class VFS {
   constructor() {
	console.log("got here today")
	this.openFiles = new Map();
        this.nextFd = 1;
        this.msgId = 1;

	let completed = false;
        let error = null;

	console.log("let's go")
	   blockUntilResolved(asyncTask())
        console.log("first wait");	

	let pr = navigator.storage.getDirectory().then(dir => {
            console.log("got directory");
            this.root = dir;
            completed = true;
        }).catch(err => {
            error = err;
            completed = true;
            console.error("failed to get directory", err);
        });
	blockUntilResolved(pr);
	console.log("some progress");

   }
   open(path, flags) {
       console.log("open", path, flags);
       const fd = this.nextFd++;
       
       try {
           // Get file handle synchronously
           const fileHandle = this.root.getFileHandleSync(path, { create: true });
           // Get sync access handle
           const accessHandle = fileHandle.createSyncAccessHandle();
           
           this.openFiles.set(fd, accessHandle);
           return fd;
       } catch (error) {
           console.error('VFS open error:', error);
           throw error;
       }
   }

   pread(fd, buffer, offset) {
       const handle = this.openFiles.get(fd);
       if (!handle) return 0;

       try {
           return handle.read(buffer, { at: offset });
       } catch (error) {
           console.error('VFS read error:', error);
           return 0;
       }
   }

   pwrite(fd, buffer, offset) {
       const handle = this.openFiles.get(fd);
       if (!handle) return 0;

       try {
           return handle.write(buffer, { at: offset });
       } catch (error) {
           console.error('VFS write error:', error);
           return 0;
       }
   }

   size(fd) {
       const handle = this.openFiles.get(fd);
       if (!handle) return BigInt(0);

       try {
           return BigInt(handle.getSize());
       } catch (error) {
           console.error('VFS size error:', error);
           return BigInt(0);
       }
   }

   sync(fd) {
       const handle = this.openFiles.get(fd);
       if (handle) {
           try {
               handle.flush();
           } catch (error) {
               console.error('VFS sync error:', error);
           }
       }
   }

   close(fd) {
       const handle = this.openFiles.get(fd);
       if (handle) {
           try {
               handle.close();
               this.openFiles.delete(fd);
           } catch (error) {
               console.error('VFS close error:', error);
           }
       }
   }
}
export { VFS };
