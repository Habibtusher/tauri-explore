'use client';
import { useState, useEffect } from 'react';

const InputDisplay = () => {
  const [inputText, setInputText] = useState('');
  const [displayText, setDisplayText] = useState('');
  const [isTauri, setIsTauri] = useState(false);

  useEffect(() => {
    // Check if we're running in Tauri
    setIsTauri(typeof window !== 'undefined' && window.__TAURI__);
  }, []);

  const handleSubmit = async () => {
    if (inputText.trim()) {
      try {
        if (isTauri) {
          // Running in Tauri - use the invoke function
          const { invoke } = await import('@tauri-apps/api/core');
          const result = await invoke('process_text', { text: inputText });
          setDisplayText(result);
        } else {
          setDisplayText(`Web fallback: Hello, ${inputText}!`);
        }
      } catch (error) {
        console.error('Error invoking Tauri command:', error);
        setDisplayText('Error processing text');
      }
    }
  };

  return (
    <div className="max-w-md mx-auto p-4">
      <div className="mb-4 text-sm text-gray-600">
        {isTauri ? 'Running in Tauri app' : 'Running in web browser'}
      </div>
      
      <input
        type="text"
        placeholder="Enter text"
        value={inputText}
        onChange={(e) => setInputText(e.target.value)}
        className="w-full p-2 mb-4 border rounded"
      />
      
      <button
        onClick={handleSubmit}
        className="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700"
      >
        Submit
      </button>
      
      {displayText && (
        <p className="mt-4 text-gray-800">{displayText}</p>
      )}
    </div>
  );
};

export default InputDisplay;