import './index.scss';
import Code from './Code';

// Initialize the code editor when the page loads
document.addEventListener('DOMContentLoaded', () => {
    const appElement = document.getElementById('app');
    if (appElement) {
        const codeEditor = new Code(appElement, 1);
        codeEditor.open();
    }
});
