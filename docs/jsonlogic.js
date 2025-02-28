import init, { JsJsonLogic } from './pkg/datalogic_rs.js';

export class JsonLogicEvaluator {
    constructor() {
        this.logic = null;
        this.initialized = false;
    }

    async initialize() {
        if (!this.initialized) {
            await init();
            this.logic = new JsJsonLogic();
            this.initialized = true;
        }
    }

    async evaluate(rules, data) {
        if (!this.initialized) {
            await this.initialize();
        }

        try {
            // Validate inputs
            if (!rules || typeof rules !== 'object') {
                throw new Error('Invalid rules format');
            }
            if (!data || typeof data !== 'object') {
                throw new Error('Invalid data format');
            }

            // Apply logic rules
            const result = await this.logic.apply(rules, data);
            return {
                success: true,
                result: result
            };
        } catch (error) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    formatJson(obj) {
        try {
            return JSON.stringify(obj, null, 2);
        } catch (error) {
            return String(obj);
        }
    }

    validateJson(jsonString) {
        try {
            JSON.parse(jsonString);
            return true;
        } catch (error) {
            return false;
        }
    }

    getSampleRules() {
        return {
            "some": [
                {"var": "items"},
                {">=": [{"var": "qty"}, 1]}
            ]
        };
    }

    getSampleData() {
        return {
            "items": [
                {"qty": 1, "id": "first"},
                {"qty": 2, "id": "second"}
            ]
        };
    }
}