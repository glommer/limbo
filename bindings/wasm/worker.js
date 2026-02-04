import init, { Database } from './pkg/limbo_wasm.js';

// Initialize the database
async function initDB() {
    try {
        await init(); // Initialize the WASM module
        
	console.log("get db");
        const db = new Database();
        await db.init("hello.db");
	console.log("init db done");
        postMessage({ message: 'Database initialized and ready' });
        return db;
    } catch (e) {
        postMessage({ message: `Database initialization failed: ${e.message}` });
        throw e;
    }
}

let dbPromise = initDB();

onmessage = async function(e) {
    try {
        const db = await dbPromise;
        
        switch (e.data.command) {
            case 'runQuery':
                try {
                    const stmt = db.prepare(e.data.sql);
                    const results = stmt.all();
                    postMessage({ 
                        message: 'Query completed successfully',
                        results: results 
                    });
                } catch (e) {
                    postMessage({ 
                        message: `Query error: ${e.message}`,
                        error: e.message
                    });
                }
                break;
        }
    } catch (e) {
        postMessage({ 
            message: `Worker error: ${e.message}`,
            error: e.message
        });
    }
};
