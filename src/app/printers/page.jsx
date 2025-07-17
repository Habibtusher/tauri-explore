"use client";
import { invoke } from '@tauri-apps/api/core';
import React, { useEffect, useState } from 'react';

const Page = () => {
    const [printers, setPrinters] = useState([]);

    useEffect(() => {
        const fetchPrinters = async () => {
            try {
                const printers = await invoke('list_printers');
                const printerList = await invoke('list_all_printers');
                console.log('Connected printers list:', printerList);
                setPrinters(printerList);
            } catch (error) {
                console.error('Error fetching printers:', error);
                setPrinters([]);
            }
        };
        fetchPrinters();
    }, []);
    console.log(printers)
    return (
        <div>
            <h1>Printers</h1>
            <table className="w-full border border-gray-300">
                <thead>
                    <tr className="bg-gray-100">
                        <th className="border border-gray-300 px-4 py-2 text-left">#</th>
                        <th className="border border-gray-300 px-4 py-2 text-left">Name</th>
                        <th className="border border-gray-300 px-4 py-2 text-left">IP Address</th>
                        <th className="border border-gray-300 px-4 py-2 text-left">Port</th>
                    </tr>
                </thead>
                <tbody>
                    {printers.length > 0 ? (
                        printers.map((printer, index) => (
                            <tr key={index}>
                                <td className="border border-gray-300 px-4 py-2">{index + 1}</td>
                                <td className="border border-gray-300 px-4 py-2">{printer.name}</td>
                                <td className="border border-gray-300 px-4 py-2">{printer.ip_address}</td>
                                <td className="border border-gray-300 px-4 py-2">{printer.port}</td>
                            </tr>
                        ))
                    ) : (
                        <tr>
                            <td className="border border-gray-300 px-4 py-2" colSpan="3">
                                No printers found
                            </td>
                        </tr>
                    )}
                </tbody>
            </table>
        </div>

    );
};

export default Page;